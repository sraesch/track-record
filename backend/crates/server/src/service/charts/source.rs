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
use sqlx::PgPool;
use track_record_core::chart::spec::SourceNode;
use track_record_core::chart::{Attr, AttrValue, Attributes, Column, Frame, Setting, Tree};
use track_record_core::dsl::Kind;
use track_record_core::dsl::eval::{EvalError, Value as DslValue};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::service::analysis::{
    CHUNK, Dataset, Excluded, MAX_SAMPLES, Restrict, RunRef, SampleRow, TestCaseRow,
    load_dataset_up_to,
};
use crate::service::derived;
use crate::service::query::Aliases;
use crate::service::runs::RunFilter;

/// Points that one run of a chart may hold, over all its sources, unless
/// the server is configured otherwise (`TR_CHART_MAX_POINTS`).
pub const DEFAULT_MAX_POINTS: usize = 10_000_000;

/// The limits of the sources of one run of a chart.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Samples per source.
    pub max_samples: i64,
    /// Points over all sources.
    pub max_points: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_samples: MAX_SAMPLES,
            max_points: DEFAULT_MAX_POINTS,
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

    fn take(&mut self, source: &str, points: usize) -> ApiResult<()> {
        self.used += points;
        if self.used > self.max {
            return Err(ApiError::invalid(format!(
                "source {source}: the chart reads more than {} points; narrow the filter of a \
                 source, or reduce its series in the metric expression, e.g. max(series.fps)",
                self.max
            )));
        }
        Ok(())
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

/// Loads the frames of the bound source `id`. `progress` is called with
/// the number of samples done after each chunk.
pub async fn load(
    pool: &PgPool,
    id: &str,
    node: &SourceNode,
    limits: &Limits,
    budget: &mut Budget,
    progress: &mut (dyn FnMut(usize) + Send),
) -> ApiResult<Loaded> {
    let metric = bound(id, "metric", &node.metric)?;
    let unit = match &node.unit {
        Some(s) => Some(bound(id, "unit", s)?.as_str()),
        None => None,
    };
    let include_warmup = match &node.include_warmup {
        Some(s) => *bound(id, "include_warmup", s)?,
        None => false,
    };
    let filter = run_filter(id, node)?;
    let prepared = derived::prepare(pool, metric, unit)
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
    let aliases = Aliases::load(pool).await?;
    let ds = load_dataset_up_to(
        pool,
        &filter,
        include_warmup,
        &Restrict::default(),
        &aliases,
        limits.max_samples,
    )
    .await
    .map_err(|e| e.context(&format!("source {id}")))?;
    let attrs = SampleAttributes::load(pool, &ds, id, &prepared.expression).await?;

    let columns = if series {
        vec![
            Column::new("x", x_unit.clone()),
            Column::new("y", prepared.signature.unit.clone()),
        ]
    } else {
        vec![Column::new("y", prepared.signature.unit.clone())]
    };
    let mut excluded = Excluded::default();
    let mut frames = Vec::new();
    let mut evaluator = prepared.evaluator();
    let mut done = 0;
    for chunk in ds.samples.chunks(CHUNK) {
        let ids: Vec<Uuid> = chunk.iter().map(|s| s.sample_id).collect();
        let data = derived::load_sample_data(pool, &ids, evaluator.inputs()).await?;
        for s in chunk {
            let out = match evaluator.evaluate(&data[&s.sample_id]) {
                Ok(out) => out,
                Err(e) => {
                    excluded.record(&e);
                    continue;
                }
            };
            let values = match out.value {
                DslValue::Scalar(v) => vec![vec![v]],
                DslValue::Series(None) => vec![vec![], vec![]],
                DslValue::Series(Some(points)) => {
                    let Some(f) = prepared
                        .catalogue
                        .conversion_factor(&out.signature.x_unit, &x_unit)
                    else {
                        excluded.record(&EvalError::Data(format!(
                            "x is in {:?}, which cannot be converted to {x_unit:?}",
                            out.signature.x_unit
                        )));
                        continue;
                    };
                    let points = points.scale_x(f);
                    vec![
                        points.x.into_iter().map(Some).collect(),
                        points.y.into_iter().map(Some).collect(),
                    ]
                }
            };
            budget.take(id, values[0].len())?;
            frames.push(Frame::new(attrs.of(s), columns.clone(), values));
        }
        done += chunk.len();
        progress(done);
        tokio::task::yield_now().await;
    }
    Ok(Loaded {
        tree: Tree::Frames(frames),
        excluded,
        samples: ds.samples.len(),
    })
}

/// The attributes of the samples of a data set, built once per run and per
/// test case.
struct SampleAttributes {
    runs: HashMap<Uuid, Attributes>,
    cases: HashMap<i64, Attributes>,
    /// `source` and `metric`.
    source: Attributes,
}

/// Run attributes that the data set does not load.
struct RunExtra {
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
    trigger: crate::db::RunTrigger,
    suite: String,
    suite_version: i32,
    tags: Vec<String>,
}

impl SampleAttributes {
    async fn load(pool: &PgPool, ds: &Dataset, id: &str, expression: &str) -> ApiResult<Self> {
        let run_ids: Vec<Uuid> = ds.runs.keys().copied().collect();
        let rows: Vec<(
            Uuid,
            Option<chrono::DateTime<chrono::Utc>>,
            crate::db::RunTrigger,
            String,
            i32,
            Vec<String>,
        )> = sqlx::query_as(
            "SELECT r.id, r.ended_at, r.triggered_by, s.name, sv.version, \
                    coalesce(array_agg(t.tag ORDER BY t.tag) FILTER (WHERE t.tag IS NOT NULL), \
                             '{}') \
             FROM run r \
             JOIN suite_version sv ON sv.id = r.suite_version_id \
             JOIN suite s ON s.id = sv.suite_id \
             LEFT JOIN run_tag t ON t.run_id = r.id \
             WHERE r.id = ANY($1) GROUP BY r.id, s.name, sv.version",
        )
        .bind(&run_ids)
        .fetch_all(pool)
        .await?;
        let extra: HashMap<Uuid, RunExtra> = rows
            .into_iter()
            .map(|(id, ended_at, trigger, suite, suite_version, tags)| {
                (
                    id,
                    RunExtra {
                        ended_at,
                        trigger,
                        suite,
                        suite_version,
                        tags,
                    },
                )
            })
            .collect();
        let runs = ds
            .runs
            .iter()
            .map(|(id, run)| (*id, run_attributes(run, extra.get(id))))
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

    fn of(&self, s: &SampleRow) -> Attributes {
        let mut a = self.source.clone();
        if let Some(run) = self.runs.get(&s.run_id) {
            a.extend(run.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
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
        a
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

fn run_attributes(run: &RunRef, extra: Option<&RunExtra>) -> Attributes {
    let mut a = BTreeMap::new();
    a.insert("run.id".into(), text(&run.id.to_string()));
    a.insert(
        "run.started_at".into(),
        Attr::new(AttrValue::Time(run.started_at)),
    );
    a.insert("run.status".into(), name_of(run.status));
    a.insert("run.valid".into(), Attr::new(AttrValue::Bool(run.is_valid)));
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
    if let Some(x) = extra {
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
    }
    a
}

/// The attributes of a case: scenario, assets and parameters after aliases.
/// Parameters keep their JSON type and their unit. An asset's value is its
/// version, or its content hash, or both as `version (hash)`.
fn case_attributes(tc: &TestCaseRow) -> Attributes {
    let mut a = BTreeMap::new();
    a.insert("scenario.name".into(), text(&tc.scenario));
    a.insert("scenario.version".into(), number(tc.scenario_version));
    for asset in &tc.assets {
        let value = match (&asset.version, &asset.content_hash) {
            (Some(v), Some(h)) => format!("{v} ({h})"),
            (Some(v), None) => v.clone(),
            (None, Some(h)) => h.clone(),
            (None, None) => asset.id.to_string(),
        };
        a.insert(format!("asset.{}", asset.name), text(&value));
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
