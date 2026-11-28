//! The source of a chart (ADR 0022, "Sources", FR-11, FR-12a, FR-14): a run
//! filter and a metric expression give one frame per sample.
//!
//! A source uses what the analysis uses: the run filter and the selection
//! of samples ([`load_dataset_up_to`]: invalid runs and warmup samples are
//! left out unless asked for, superseded case executions never count,
//! aliases are applied), and the evaluation of the expression per sample
//! (`service::derived`), with series read through the series store.
//!
//! - A scalar gives a frame with one point and the column `y`; a derived
//!   scalar without a value is that point with no value. A series gives the
//!   columns `x` and `y`, with `x` in the source's `x_unit`; a derived series
//!   without a value is a frame without points.
//! - Samples that lack an input of the expression, or whose inputs do not
//!   fit it, get no frame; they are counted in `excluded`.
//! - Frames are ordered by measured start of the run, run, case execution
//!   and repetition, and carry every attribute of their sample.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;
use sqlx::PgConnection;
use track_record_core::chart::spec::SourceNode;
use track_record_core::chart::{Attr, AttrValue, Attributes, Column, Frame, Setting, Tree};
use track_record_core::dsl::eval::{EvalError, Value as DslValue};
use track_record_core::dsl::{Kind, References};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::service::analysis::{
    CHUNK, Dataset, Excluded, MAX_SAMPLES, Restrict, RunRef, SampleRow, TestCaseRow,
    load_dataset_up_to,
};
use crate::service::derived::{self, Prepared};
use crate::service::query::Aliases;
use crate::service::runs::{AssetRef, RunFilter};

/// Points that one run of a chart may hold, over all its sources, unless
/// the server is configured otherwise (`TR_CHART_MAX_POINTS`).
pub const DEFAULT_MAX_POINTS: usize = 10_000_000;

/// Points of the input series that a source loads at a time.
pub const BATCH_POINTS: usize = 1_000_000;

/// The limits of the sources of one run of a chart.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Samples per source.
    pub max_samples: i64,
    /// Points over all sources: the frames made and the input points of the
    /// batch being loaded.
    pub max_points: usize,
    /// Points of input series loaded at a time, at most what the budget has
    /// left; a sample with more is loaded alone.
    pub batch_points: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_samples: MAX_SAMPLES,
            max_points: DEFAULT_MAX_POINTS,
            batch_points: BATCH_POINTS,
        }
    }
}

/// The points that the sources of one run may still load.
#[derive(Debug)]
pub struct Budget {
    max: usize,
    used: usize,
}

impl Budget {
    pub fn new(max_points: usize) -> Self {
        Self {
            max: max_points,
            used: 0,
        }
    }

    /// Points loaded so far.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Points that may still be loaded.
    pub fn room(&self) -> usize {
        self.max.saturating_sub(self.used)
    }

    fn take(&mut self, source: &str, points: usize) -> ApiResult<()> {
        self.used += points;
        if self.used > self.max {
            return Err(self.exceeded(source));
        }
        Ok(())
    }

    fn exceeded(&self, source: &str) -> ApiError {
        ApiError::invalid(format!(
            "source {source}: the chart reads more than {} points; narrow the filter of a \
             source, or reduce its series in the metric expression, e.g. max(series.fps)",
            self.max
        ))
    }
}

/// The frames of a source.
#[derive(Debug)]
pub struct Loaded {
    pub tree: Tree,
    /// Samples without a frame because the metric does not apply to them.
    pub excluded: Excluded,
    /// Samples the filter selected.
    pub samples: usize,
}

/// The value of a setting of a bound source.
fn bound<'a, T>(id: &str, setting: &str, s: &'a Setting<T>) -> ApiResult<&'a T> {
    match s {
        Setting::Value(v) => Ok(v),
        Setting::Var(name) => Err(ApiError::invalid(format!(
            "source {id}: {setting}: variable {name:?} has no value"
        ))),
    }
}

/// The server's run filter of a bound source.
pub fn run_filter(id: &str, node: &SourceNode) -> ApiResult<RunFilter> {
    let Some(filter) = &node.filter else {
        return Ok(RunFilter::default());
    };
    let mut fields = serde_json::Map::new();
    for (name, value) in bound(id, "filter", filter)? {
        let value = bound(id, &format!("filter.{name}"), value)?;
        fields.insert(name.clone(), value.clone());
    }
    serde_json::from_value(Value::Object(fields))
        .map_err(|e| ApiError::invalid(format!("source {id}: filter: {e}")))
}

/// Loads the frames of the bound source `id`: [`select`], then
/// [`Selected::load`]. `progress` is called with the number of samples done
/// after each chunk.
pub async fn load(
    conn: &mut PgConnection,
    id: &str,
    node: &SourceNode,
    limits: &Limits,
    budget: &mut Budget,
    progress: &mut (dyn FnMut(usize) + Send),
) -> ApiResult<Loaded> {
    select(conn, id, node, limits, false)
        .await?
        .load(conn, limits, budget, progress)
        .await
}

/// The samples a source selects and their attributes, before any series is
/// read. With `preview`, only the latest run that matches the filter.
pub struct Selected {
    id: String,
    prepared: Prepared,
    x_unit: String,
    ds: Dataset,
    attrs: SampleAttributes,
}

impl std::fmt::Debug for Selected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selected")
            .field("id", &self.id)
            .field("samples", &self.ds.samples.len())
            .finish_non_exhaustive()
    }
}

/// Selects the samples of the bound source `id` (the first phase of
/// [`load`]). Errors of the source and the limit of samples show here,
/// before any point is loaded.
pub async fn select(
    conn: &mut PgConnection,
    id: &str,
    node: &SourceNode,
    limits: &Limits,
    preview: bool,
) -> ApiResult<Selected> {
    let metric = bound(id, "metric", &node.metric)?;
    let unit = match &node.unit {
        Some(s) => Some(bound(id, "unit", s)?.as_str()),
        None => None,
    };
    let include_warmup = match &node.include_warmup {
        Some(s) => *bound(id, "include_warmup", s)?,
        None => false,
    };
    let mut filter = run_filter(id, node)?;
    if preview {
        filter.latest_runs = Some(1);
    }
    let prepared = derived::prepare(conn, metric, unit)
        .await
        .map_err(|e| e.context(&format!("source {id}")))?;
    let series = prepared.signature.kind == Kind::Series;
    let x_unit = match &node.x_unit {
        Some(_) if !series => {
            return Err(ApiError::invalid(format!(
                "source {id}: {metric} gives a scalar, which has no x; x_unit is for series"
            )));
        }
        Some(s) => bound(id, "x_unit", s)?.clone(),
        None => prepared.signature.x_unit.clone(),
    };
    if series
        && prepared
            .catalogue
            .conversion_factor(&prepared.signature.x_unit, &x_unit)
            .is_none()
    {
        return Err(ApiError::invalid(format!(
            "source {id}: x is in {:?}, which cannot be converted to {x_unit:?}",
            prepared.signature.x_unit
        )));
    }
    let aliases = Aliases::load(&mut *conn).await?;
    let ds = load_dataset_up_to(
        conn,
        &filter,
        include_warmup,
        &Restrict::default(),
        &aliases,
        limits.max_samples,
    )
    .await
    .map_err(|e| e.context(&format!("source {id}")))?;
    let attrs = SampleAttributes::load(conn, &ds, id, &prepared.expression).await?;
    Ok(Selected {
        id: id.to_owned(),
        prepared,
        x_unit,
        ds,
        attrs,
    })
}

impl Selected {
    /// Samples the filter selected.
    pub fn samples(&self) -> usize {
        self.ds.samples.len()
    }

    /// Loads the frames of the selected samples (the second phase of
    /// [`load`]), charging their points to `budget`.
    pub async fn load(
        self,
        conn: &mut PgConnection,
        limits: &Limits,
        budget: &mut Budget,
        progress: &mut (dyn FnMut(usize) + Send),
    ) -> ApiResult<Loaded> {
        let Selected {
            id,
            prepared,
            x_unit,
            ds,
            attrs,
        } = self;
        let id = id.as_str();
        let y = Column::new("y", prepared.signature.unit.clone());
        let mut excluded = Excluded::default();
        let mut frames = Vec::new();
        let mut evaluator = prepared.evaluator();
        let mut done = 0;
        for chunk in ds.samples.chunks(CHUNK) {
            // The series a chunk reads are loaded in batches of a bounded number
            // of points, so that a source of long series does not hold more
            // than a batch of input points besides the frames it made.
            let counts = point_counts(&mut *conn, chunk, evaluator.inputs()).await?;
            let mut start = 0;
            while start < chunk.len() {
                // A batch fits into what the budget has left, so that the input
                // points of a batch and the frames made so far stay within it.
                // Inputs are dropped as their sample is evaluated.
                let room = budget.room();
                let end = next_batch(&counts, start, limits.batch_points.min(room), room).map_err(
                    |i| {
                        ApiError::invalid(format!(
                            "source {id}: sample {} has {} points in the series the metric reads, \
                         more than the {room} points the chart may still hold; narrow the \
                         filter of the source, or reduce its series in the metric expression, \
                         e.g. max(series.fps)",
                            chunk[i].sample_id, counts[i]
                        ))
                    },
                )?;
                let batch = &chunk[start..end];
                start = end;
                let ids: Vec<Uuid> = batch.iter().map(|s| s.sample_id).collect();
                let mut data = derived::load_sample_data(conn, &ids, evaluator.inputs()).await?;
                for s in batch {
                    let input = data.remove(&s.sample_id).unwrap_or_default();
                    // A run deleted while the source loads has no attributes
                    // any more; its samples are gone with it, before they are
                    // evaluated, counted or charged to the budget.
                    let Some(attrs) = attrs.of(s) else {
                        continue;
                    };
                    let out = evaluator.evaluate(&input);
                    drop(input);
                    let out = match out {
                        Ok(out) => out,
                        Err(e) => {
                            excluded.record(&e);
                            continue;
                        }
                    };
                    // A series, with or without a value, needs an x that
                    // converts to the source's x unit.
                    let factor = match &out.value {
                        DslValue::Scalar(_) => 1.0,
                        DslValue::Series(_) => match prepared
                            .catalogue
                            .conversion_factor(&out.signature.x_unit, &x_unit)
                        {
                            Some(f) => f,
                            None => {
                                excluded.record(&EvalError::Data(format!(
                                    "x is in {:?}, which cannot be converted to {x_unit:?}",
                                    out.signature.x_unit
                                )));
                                continue;
                            }
                        },
                    };
                    let x = || Column::x(x_unit.clone(), out.signature.x_dimension.clone());
                    let (columns, values) = match out.value {
                        DslValue::Scalar(v) => {
                            budget.take(id, 1)?;
                            (vec![y.clone()], vec![vec![v]])
                        }
                        DslValue::Series(None) => (vec![x(), y.clone()], vec![vec![], vec![]]),
                        DslValue::Series(Some(mut points)) => {
                            if factor != 1.0 {
                                for x in &mut points.x {
                                    *x *= factor;
                                }
                            }
                            // A conversion can overflow (1e300 s in ns).
                            if let Some(x) = points.x.iter().find(|x| !x.is_finite()) {
                                excluded.record(&EvalError::Data(format!(
                                    "x is {x} in {x_unit:?}, which cannot be shown"
                                )));
                                continue;
                            }
                            budget.take(id, points.len())?;
                            (
                                vec![x(), y.clone()],
                                vec![
                                    points.x.into_iter().map(Some).collect(),
                                    points.y.into_iter().map(Some).collect(),
                                ],
                            )
                        }
                    };
                    frames.push(Frame::new(attrs, columns, values));
                }
                tokio::task::yield_now().await;
            }
            done += chunk.len();
            progress(done);
        }
        Ok(Loaded {
            tree: Tree::Frames(frames),
            excluded,
            samples: ds.samples.len(),
        })
    }
}

/// The points of the series the metric reads, per sample of `chunk` (in
/// its order), by the point counts stored with the series.
async fn point_counts(
    executor: impl sqlx::PgExecutor<'_>,
    chunk: &[SampleRow],
    refs: &References,
) -> ApiResult<Vec<usize>> {
    if refs.series.is_empty() {
        return Ok(vec![0; chunk.len()]);
    }
    let ids: Vec<Uuid> = chunk.iter().map(|s| s.sample_id).collect();
    let names: Vec<&String> = refs.series.iter().collect();
    let counts: HashMap<Uuid, i64> = sqlx::query_as(
        "SELECT se.sample_id, sum(se.point_count)::bigint FROM series se \
         JOIN metric mt ON mt.id = se.metric_id \
         WHERE se.sample_id = ANY($1) AND mt.name = ANY($2) GROUP BY se.sample_id",
    )
    .bind(&ids)
    .bind(names)
    .fetch_all(executor)
    .await?
    .into_iter()
    .collect();
    Ok(chunk
        .iter()
        .map(|s| {
            let n = counts.get(&s.sample_id).copied().unwrap_or(0);
            usize::try_from(n).unwrap_or(usize::MAX)
        })
        .collect())
}

/// What a sample takes of the budget while its batch is loaded: its input
/// points, at least the one point it makes when it has none (a scalar).
fn cost(points: usize) -> usize {
    points.max(1)
}

/// The end of the batch that starts at `start`: the samples whose input
/// series (`counts`) take at most `limit` points together ([`cost`]), at
/// least one. `Err` is a sample that alone takes more than the `room` left
/// in the budget: it fails before it is loaded, as an expression needs all
/// inputs of a sample at once.
fn next_batch(counts: &[usize], start: usize, limit: usize, room: usize) -> Result<usize, usize> {
    let mut points = 0usize;
    for (i, &inputs) in counts.iter().enumerate().skip(start) {
        let n = cost(inputs);
        if inputs > room {
            return Err(i);
        }
        if n > room {
            // No room for the point a sample without inputs may make: the
            // samples without inputs from here are a batch, and the budget
            // fails at the first that makes a point (one may have no value,
            // or lack the metric). Their inputs are scalars.
            if i > start {
                return Ok(i);
            }
            let end = counts[i..]
                .iter()
                .position(|&c| c > 0)
                .map_or(counts.len(), |p| i + p);
            return Ok(end);
        }
        if i > start && points.saturating_add(n) > limit {
            return Ok(i);
        }
        points = points.saturating_add(n);
    }
    Ok(counts.len())
}

/// The attributes of the samples of a data set, built once per run and per
/// test case.
struct SampleAttributes {
    runs: HashMap<Uuid, Attributes>,
    cases: HashMap<i64, Attributes>,
    /// `source` and `metric`.
    source: Attributes,
}

/// Run attributes that the data set does not load, and those that change
/// while a run is uploaded, read together from one row.
struct RunExtra {
    status: crate::db::RunStatus,
    is_valid: bool,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
    trigger: crate::db::RunTrigger,
    suite: String,
    suite_version: i32,
    tags: Vec<String>,
}

impl SampleAttributes {
    async fn load(
        executor: impl sqlx::PgExecutor<'_>,
        ds: &Dataset,
        id: &str,
        expression: &str,
    ) -> ApiResult<Self> {
        let run_ids: Vec<Uuid> = ds.runs.keys().copied().collect();
        let rows: Vec<(
            Uuid,
            crate::db::RunStatus,
            bool,
            Option<chrono::DateTime<chrono::Utc>>,
            crate::db::RunTrigger,
            String,
            i32,
            Vec<String>,
        )> = sqlx::query_as(
            "SELECT r.id, r.status, r.is_valid, r.ended_at, r.triggered_by, s.name, sv.version, \
                    coalesce(array_agg(t.tag ORDER BY t.tag) FILTER (WHERE t.tag IS NOT NULL), \
                             '{}') \
             FROM run r \
             JOIN suite_version sv ON sv.id = r.suite_version_id \
             JOIN suite s ON s.id = sv.suite_id \
             LEFT JOIN run_tag t ON t.run_id = r.id \
             WHERE r.id = ANY($1) GROUP BY r.id, s.name, sv.version",
        )
        .bind(&run_ids)
        .fetch_all(executor)
        .await?;
        let extra: HashMap<Uuid, RunExtra> = rows
            .into_iter()
            .map(
                |(id, status, is_valid, ended_at, trigger, suite, suite_version, tags)| {
                    (
                        id,
                        RunExtra {
                            status,
                            is_valid,
                            ended_at,
                            trigger,
                            suite,
                            suite_version,
                            tags,
                        },
                    )
                },
            )
            .collect();
        let runs = ds
            .runs
            .iter()
            .filter_map(|(id, run)| Some((*id, run_attributes(run, extra.get(id)?))))
            .collect();
        let cases = ds
            .test_cases
            .iter()
            .map(|(id, tc)| (*id, case_attributes(tc)))
            .collect();
        let mut source = Attributes::new();
        source.insert("source".into(), text(id));
        source.insert("metric".into(), text(expression));
        Ok(Self {
            runs,
            cases,
            source,
        })
    }

    /// The attributes of a sample; `None` if its run is gone.
    fn of(&self, s: &SampleRow) -> Option<Attributes> {
        let mut a = self.source.clone();
        let run = self.runs.get(&s.run_id)?;
        a.extend(run.iter().map(|(k, v)| (k.clone(), v.clone())));
        if let Some(case) = self.cases.get(&s.test_case_id) {
            a.extend(case.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        a.insert("device.name".into(), text(&s.client_device));
        a.insert("device.details".into(), number(s.device_environment_id));
        a.insert(
            "case_execution.id".into(),
            text(&s.case_execution_id.to_string()),
        );
        a.insert("sample.id".into(), text(&s.sample_id.to_string()));
        a.insert("sample.repetition".into(), number(s.repetition));
        a.insert(
            "sample.warmup".into(),
            Attr::new(AttrValue::Bool(s.is_warmup)),
        );
        Some(a)
    }
}

fn text(s: &str) -> Attr {
    Attr::new(AttrValue::Text(s.to_owned()))
}

fn number(n: impl Into<serde_json::Number>) -> Attr {
    Attr::new(AttrValue::Number(n.into()))
}

/// The text form of an enumeration, as the API writes it.
fn name_of(v: impl serde::Serialize) -> Attr {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => text(&s),
        other => text(&format!("{other:?}")),
    }
}

/// The version and content hash of an asset, tagged.
fn asset_parts(asset: &AssetRef) -> AttrValue {
    let mut parts = Vec::new();
    if let Some(v) = &asset.version {
        parts.push(AttrValue::Text(format!("version={v}")));
    }
    if let Some(h) = &asset.content_hash {
        parts.push(AttrValue::Text(format!("hash={h}")));
    }
    AttrValue::List(parts)
}

fn run_attributes(run: &RunRef, x: &RunExtra) -> Attributes {
    let mut a = BTreeMap::new();
    a.insert("run.id".into(), text(&run.id.to_string()));
    a.insert(
        "run.started_at".into(),
        Attr::new(AttrValue::Time(run.started_at)),
    );
    // Status, validity and end time change while a run is uploaded or
    // invalidated; they come from one read of the run's row.
    a.insert("run.status".into(), name_of(x.status));
    a.insert("run.valid".into(), Attr::new(AttrValue::Bool(x.is_valid)));
    a.insert("environment.id".into(), number(run.shared_environment_id));
    let sut = &run.sut_version;
    a.insert("sut.id".into(), number(sut.id));
    a.insert("sut.commit".into(), text(&sut.commit));
    for (name, value) in [
        ("sut.branch", &sut.branch),
        ("sut.build_config", &sut.build_config),
        ("sut.tag", &sut.tag),
    ] {
        if let Some(v) = value {
            a.insert(name.into(), text(v));
        }
    }
    if let Some(t) = x.ended_at {
        a.insert("run.ended_at".into(), Attr::new(AttrValue::Time(t)));
    }
    a.insert("run.trigger".into(), name_of(x.trigger));
    a.insert("suite.name".into(), text(&x.suite));
    a.insert("suite.version".into(), number(x.suite_version));
    a.insert(
        "run.tags".into(),
        Attr::new(AttrValue::List(
            x.tags.iter().map(|t| AttrValue::Text(t.clone())).collect(),
        )),
    );
    a
}

/// The attributes of a case: scenario, assets and parameters after aliases.
/// Parameters keep their JSON type and their unit.
///
/// `asset.<name>` is the version of the asset when the case has one asset
/// of that name and it has only a version, the common case. One asset with
/// a content hash is a list of `version=<version>` and `hash=<content
/// hash>`, and several assets of the name a list of such lists, in order:
/// distinct assets never give the same value, whatever their versions say.
fn case_attributes(tc: &TestCaseRow) -> Attributes {
    let mut a = BTreeMap::new();
    a.insert("scenario.name".into(), text(&tc.scenario));
    a.insert("scenario.version".into(), number(tc.scenario_version));
    let mut assets: BTreeMap<&str, Vec<&AssetRef>> = BTreeMap::new();
    for asset in &tc.assets {
        assets.entry(&asset.name).or_default().push(asset);
    }
    for (name, list) in assets {
        let value = match list.as_slice() {
            [
                AssetRef {
                    version: Some(v),
                    content_hash: None,
                    ..
                },
            ] => AttrValue::Text(v.clone()),
            [one] => asset_parts(one),
            many => {
                let mut values: Vec<AttrValue> = many.iter().map(|x| asset_parts(x)).collect();
                values.sort_by(AttrValue::total_cmp);
                AttrValue::List(values)
            }
        };
        a.insert(format!("asset.{name}"), Attr::new(value));
    }
    for (name, value) in &tc.parameters {
        let Some(v) = AttrValue::from_json(value) else {
            continue;
        };
        let unit = tc
            .parameter_units
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default();
        a.insert(format!("param.{name}"), Attr::with_unit(v, unit));
    }
    a
}

#[cfg(test)]
mod tests {
    use super::next_batch;

    #[test]
    fn a_sample_without_inputs_takes_a_point() {
        use super::cost;
        // An empty series before a five-point one: six points with a room
        // of five, so the second sample does not fit after the first.
        assert_eq!(next_batch(&[0, 5], 0, 5, 5), Ok(1));
        assert_eq!(next_batch(&[0, 5], 1, 4, 4), Err(1));
        assert_eq!(cost(0), 1);
        // Without room, a sample without inputs is a batch of its own: the
        // budget fails only if it makes a point.
        assert_eq!(next_batch(&[0, 0], 0, 1_000, 0), Ok(2));
        assert_eq!(next_batch(&[0, 0, 4, 0], 0, 1_000, 0), Ok(2));
        assert_eq!(next_batch(&[0, 0, 0], 1, 1, 1), Ok(2));
        assert_eq!(next_batch(&[0, 0, 0], 2, 1_000, 0), Ok(3));
    }

    #[test]
    fn batches_fit_the_batch_size_and_the_room_left() {
        let counts = [500, 500, 500, 0, 0, 300];
        // The batch size is the smaller of the two limits.
        assert_eq!(next_batch(&counts, 0, 1_000_000, 1_000_000), Ok(6));
        assert_eq!(next_batch(&counts, 0, 1_000, 1_000), Ok(2));
        assert_eq!(next_batch(&counts, 2, 600, 600), Ok(5));
        // A sample over the batch size is a batch of its own.
        assert_eq!(next_batch(&counts, 0, 100, 1_000), Ok(1));
        // A sample over the room left fails.
        assert_eq!(next_batch(&counts, 1, 400, 400), Err(1));
        // With no room, a sample without inputs is a batch of its own; one
        // with inputs fails.
        assert_eq!(next_batch(&counts, 3, 0, 0), Ok(5));
        assert_eq!(next_batch(&counts, 5, 0, 0), Err(5));
        assert_eq!(next_batch(&[], 0, 10, 10), Ok(0));
    }
}
