//! Analysis (FR-12 to FR-21): aggregates across samples, trends, comparisons,
//! series overlays and exports.
//!
//! Every analysis takes a JSON view spec that the UI can keep in its URL
//! (FR-21): a run filter (FR-11), the metric as a DSL expression (an
//! uploaded metric such as `load_time`, a derived metric such as
//! `derived.throughput`, or any expression; FR-17a), an optional unit
//! (FR-13) and whether warmup samples count (FR-12, excluded by default).
//! Invalid runs are excluded unless the filter's `validity` says otherwise
//! (FR-9). Superseded case executions are never used.
//!
//! Results are grouped by run × case × client device. The case key is the
//! scenario version, the assets and the parameters after aliases (with
//! their units), and devices are compared by name after aliases (FR-14), so
//! renamed values keep their trend. Samples of different runs are never
//! pooled.

pub mod compare;
pub mod export;
pub mod series;

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use track_record_core::dsl::Kind;
use track_record_core::dsl::eval::{EvalError, Value as DslValue};
use track_record_core::dsl::program::Outcome;
use track_record_core::stats::{Summary, summarize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::RunStatus;
use crate::error::{ApiError, ApiResult};
use crate::service::annotations::Annotation;
use crate::service::derived::{self, Prepared};
use crate::service::query::{Aliases, parameter_dimension};
use crate::service::runs::{
    AssetRef, RunFilter, SutVersion, push_case_execution_conditions, push_run_level_conditions,
    test_case_assets,
};

/// Percentiles reported when none are requested.
pub const DEFAULT_PERCENTILES: [f64; 4] = [5.0, 25.0, 75.0, 95.0];
/// Most samples one analysis may touch; narrow the filter beyond that.
pub const MAX_SAMPLES: i64 = 2_000_000;
/// Samples whose inputs are loaded and evaluated at a time.
pub const CHUNK: usize = 2_000;

fn default_confidence() -> f64 {
    0.95
}

// View spec parts ---------------------------------------------------------------------

/// The part of every view spec that selects samples and the metric.
#[derive(Debug, Clone)]
pub struct Selection<'a> {
    pub filter: &'a RunFilter,
    pub metric: &'a str,
    pub unit: Option<&'a str>,
    pub include_warmup: bool,
}

/// Statistics options shared by the views.
#[derive(Debug, Clone)]
pub struct StatsOptions {
    pub percentiles: Vec<f64>,
    pub confidence: f64,
}

impl StatsOptions {
    pub fn new(percentiles: &Option<Vec<f64>>, confidence: f64) -> ApiResult<Self> {
        let percentiles = percentiles
            .clone()
            .unwrap_or_else(|| DEFAULT_PERCENTILES.to_vec());
        if percentiles.iter().any(|p| !(0.0..=100.0).contains(p)) || percentiles.len() > 20 {
            return Err(ApiError::bad_request(
                "percentiles must be at most 20 values between 0 and 100",
            ));
        }
        if !(confidence > 0.0 && confidence < 1.0) {
            return Err(ApiError::bad_request("confidence must be between 0 and 1"));
        }
        Ok(Self {
            percentiles,
            confidence,
        })
    }

    pub fn summarize(&self, values: &[Option<f64>]) -> Stats {
        summarize(values, &self.percentiles, self.confidence).into()
    }
}

// Output types ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PercentileValue {
    pub p: f64,
    pub value: Option<f64>,
}

/// Aggregate across the samples of one run, case and device (FR-12).
/// Samples without a value (FR-12a) are counted in `n_no_value` and left
/// out of the statistics.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Stats {
    /// Samples with a value.
    pub n: usize,
    /// Samples without a value.
    pub n_no_value: usize,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub stddev: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub percentiles: Vec<PercentileValue>,
    /// Confidence interval of the mean.
    pub ci_low: Option<f64>,
    pub ci_high: Option<f64>,
}

impl From<Summary> for Stats {
    fn from(s: Summary) -> Self {
        Self {
            n: s.n,
            n_no_value: s.n_no_value,
            mean: s.mean,
            median: s.median,
            stddev: s.stddev,
            min: s.min,
            max: s.max,
            percentiles: s
                .percentiles
                .into_iter()
                .map(|(p, value)| PercentileValue { p, value })
                .collect(),
            ci_low: s.ci.map(|c| c.0),
            ci_high: s.ci.map(|c| c.1),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunRef {
    pub id: Uuid,
    /// Measured start time.
    pub started_at: DateTime<Utc>,
    pub status: RunStatus,
    pub is_valid: bool,
    pub sut_version: SutVersion,
    pub shared_environment_id: i64,
}

/// A case: scenario version, assets and parameters after aliases.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseInfo {
    pub scenario: String,
    pub scenario_version: i32,
    pub scenario_version_id: i64,
    pub assets: Vec<AssetRef>,
    /// Parameters after aliases (only those in the matching key, for a
    /// comparison).
    #[schema(value_type = Object)]
    pub parameters: Value,
    #[schema(value_type = Object)]
    pub parameter_units: Value,
    /// Test cases that make up this case; several when aliases merge
    /// parameter values.
    pub test_case_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricInfo {
    /// The metric expression as requested.
    pub expression: String,
    /// `scalar` or `series`.
    pub kind: String,
    /// Unit of all values.
    pub unit: String,
}

impl MetricInfo {
    fn of(p: &Prepared) -> Self {
        Self {
            expression: p.expression.clone(),
            kind: p.signature.kind.as_str().to_owned(),
            unit: p.signature.unit.clone(),
        }
    }
}

/// Samples left out because the metric does not apply to them.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct Excluded {
    /// Samples without an input of the expression (not measured).
    pub missing: usize,
    /// Samples whose inputs do not fit the expression (e.g. units).
    pub errors: usize,
    /// The first such error, for diagnosis.
    pub first_error: Option<String>,
}

impl Excluded {
    fn record(&mut self, e: &EvalError) {
        match e {
            EvalError::Missing(_) => self.missing += 1,
            EvalError::Data(m) => {
                self.errors += 1;
                if self.first_error.is_none() {
                    self.first_error = Some(m.clone());
                }
            }
        }
    }
}

// Data set ------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TestCaseRow {
    pub id: i64,
    pub scenario: String,
    pub scenario_version: i32,
    pub scenario_version_id: i64,
    /// Parameters after aliases.
    pub parameters: serde_json::Map<String, Value>,
    pub parameter_units: serde_json::Map<String, Value>,
    pub assets: Vec<AssetRef>,
}

#[derive(Debug, Clone)]
pub struct SampleRow {
    pub run_id: Uuid,
    pub case_execution_id: Uuid,
    pub test_case_id: i64,
    /// Device name after aliases.
    pub client_device: String,
    pub device_environment_id: i64,
    pub sample_id: Uuid,
    pub repetition: i32,
    pub is_warmup: bool,
}

/// The samples selected by a filter with their runs and test cases, in run
/// order (measured start time).
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    pub runs: HashMap<Uuid, RunRef>,
    pub test_cases: HashMap<i64, TestCaseRow>,
    pub samples: Vec<SampleRow>,
}

/// Extra restrictions used by the trend.
#[derive(Debug, Clone, Default)]
pub struct Restrict {
    pub scenario_version_id: Option<i64>,
    pub device_names: Option<Vec<String>>,
}

pub async fn load_dataset(
    pool: &PgPool,
    filter: &RunFilter,
    include_warmup: bool,
    restrict: &Restrict,
    aliases: &Aliases,
) -> ApiResult<Dataset> {
    if filter.limit.is_some() || filter.cursor.is_some() {
        return Err(ApiError::bad_request(
            "limit and cursor apply to the run list, not to analysis",
        ));
    }
    let mut q = QueryBuilder::<Postgres>::new(
        "SELECT r.id, ce.id, ce.test_case_id, cd.name, ce.device_environment_id, \
                sa.id, sa.repetition, sa.is_warmup \
         FROM run r \
         JOIN suite_version sv ON sv.id = r.suite_version_id \
         JOIN suite s ON s.id = sv.suite_id \
         JOIN sut_version sut ON sut.id = r.sut_version_id \
         JOIN current_case_execution ce ON ce.run_id = r.id \
         JOIN test_case tc ON tc.id = ce.test_case_id \
         JOIN scenario_version scv ON scv.id = tc.scenario_version_id \
         JOIN scenario sc ON sc.id = scv.scenario_id \
         JOIN client_device cd ON cd.id = ce.client_device_id \
         JOIN sample sa ON sa.case_execution_id = ce.id \
         WHERE true",
    );
    push_run_level_conditions(&mut q, filter);
    push_case_execution_conditions(&mut q, filter, aliases)?;
    if !include_warmup {
        q.push(" AND NOT sa.is_warmup");
    }
    if let Some(id) = restrict.scenario_version_id {
        q.push(" AND tc.scenario_version_id = ").push_bind(id);
    }
    if let Some(names) = &restrict.device_names {
        q.push(" AND cd.name = ANY(")
            .push_bind(names.clone())
            .push(")");
    }
    q.push(" ORDER BY r.started_at, r.id, ce.id, sa.repetition LIMIT ")
        .push_bind(MAX_SAMPLES + 1);
    let rows: Vec<(Uuid, Uuid, i64, String, i64, Uuid, i32, bool)> =
        q.build_query_as().fetch_all(pool).await?;
    if rows.len() as i64 > MAX_SAMPLES {
        return Err(ApiError::invalid(format!(
            "the filter selects more than {MAX_SAMPLES} samples; narrow it"
        )));
    }

    let run_ids: Vec<Uuid> = unique(rows.iter().map(|r| r.0));
    let test_case_ids: Vec<i64> = unique(rows.iter().map(|r| r.2));
    let runs = load_runs(pool, &run_ids).await?;
    let test_cases = load_test_cases(pool, &test_case_ids, aliases).await?;
    let samples = rows
        .into_iter()
        .map(
            |(
                run_id,
                case_execution_id,
                test_case_id,
                device,
                device_environment_id,
                sample_id,
                repetition,
                is_warmup,
            )| {
                SampleRow {
                    run_id,
                    case_execution_id,
                    test_case_id,
                    client_device: aliases.resolve_device(&device),
                    device_environment_id,
                    sample_id,
                    repetition,
                    is_warmup,
                }
            },
        )
        .collect();
    Ok(Dataset {
        runs,
        test_cases,
        samples,
    })
}

fn unique<T: std::hash::Hash + Eq + Copy>(items: impl Iterator<Item = T>) -> Vec<T> {
    let mut seen = HashSet::new();
    items.filter(|i| seen.insert(*i)).collect()
}

pub async fn load_runs(pool: &PgPool, ids: &[Uuid]) -> ApiResult<HashMap<Uuid, RunRef>> {
    let rows: Vec<(
        Uuid,
        DateTime<Utc>,
        RunStatus,
        bool,
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT r.id, r.started_at, r.status, r.is_valid, r.shared_environment_id, \
                sut.id, sut.commit, sut.branch, sut.build_config, sut.tag \
         FROM run r JOIN sut_version sut ON sut.id = r.sut_version_id WHERE r.id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                started_at,
                status,
                is_valid,
                shared_environment_id,
                sut,
                commit,
                branch,
                build_config,
                tag,
            )| {
                (
                    id,
                    RunRef {
                        id,
                        started_at,
                        status,
                        is_valid,
                        shared_environment_id,
                        sut_version: SutVersion {
                            id: sut,
                            commit,
                            branch,
                            build_config,
                            tag,
                        },
                    },
                )
            },
        )
        .collect())
}

pub async fn load_test_cases(
    pool: &PgPool,
    ids: &[i64],
    aliases: &Aliases,
) -> ApiResult<HashMap<i64, TestCaseRow>> {
    let rows: Vec<(i64, String, i32, i64, Value, Value)> = sqlx::query_as(
        "SELECT tc.id, sc.name, scv.version, scv.id, tc.parameters, tc.parameter_units \
         FROM test_case tc JOIN scenario_version scv ON scv.id = tc.scenario_version_id \
         JOIN scenario sc ON sc.id = scv.scenario_id WHERE tc.id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    let mut assets = test_case_assets(pool, ids).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, scenario, scenario_version, scenario_version_id, parameters, units)| {
                let object = |v: Value| match v {
                    Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
                (
                    id,
                    TestCaseRow {
                        id,
                        scenario,
                        scenario_version,
                        scenario_version_id,
                        parameters: object(aliases.resolve_parameters(&parameters)),
                        parameter_units: object(units),
                        assets: assets.remove(&id).unwrap_or_default(),
                    },
                )
            },
        )
        .collect())
}

// Case keys ------------------------------------------------------------------------------

/// A canonical text form of a JSON value: object keys sorted, numbers by
/// value (`10` equals `10.0`, as in jsonb), so equal values give equal keys.
pub fn canonical(value: &Value) -> String {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(f) => format!("{f:?}"),
            None => n.to_string(),
        },
        Value::Object(m) => {
            let sorted: BTreeMap<&String, &Value> = m.iter().collect();
            let parts: Vec<String> = sorted
                .into_iter()
                .map(|(k, v)| format!("{}:{}", Value::String(k.clone()), canonical(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

/// The parameters of a test case without `drop`.
pub fn kept_parameters(
    tc: &TestCaseRow,
    drop: &[String],
) -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let keep = |m: &serde_json::Map<String, Value>| {
        m.iter()
            .filter(|(k, _)| !drop.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    (keep(&tc.parameters), keep(&tc.parameter_units))
}

/// The case key of a test case: scenario version, assets and parameters
/// after aliases with their units, without the parameters in `drop`.
pub fn case_key(tc: &TestCaseRow, drop: &[String]) -> String {
    let (params, units) = kept_parameters(tc, drop);
    let mut assets: Vec<i64> = tc.assets.iter().map(|a| a.id).collect();
    assets.sort_unstable();
    format!(
        "{}|{:?}|{}|{}",
        tc.scenario_version_id,
        assets,
        canonical(&Value::Object(params)),
        canonical(&Value::Object(units))
    )
}

/// Collects the display form of cases as test cases are added to them.
#[derive(Debug, Default)]
pub struct Cases {
    by_key: HashMap<String, CaseInfo>,
}

impl Cases {
    pub fn add(&mut self, key: &str, tc: &TestCaseRow, drop: &[String]) {
        let info = self.by_key.entry(key.to_owned()).or_insert_with(|| {
            let (params, units) = kept_parameters(tc, drop);
            CaseInfo {
                scenario: tc.scenario.clone(),
                scenario_version: tc.scenario_version,
                scenario_version_id: tc.scenario_version_id,
                assets: tc.assets.clone(),
                parameters: Value::Object(params),
                parameter_units: Value::Object(units),
                test_case_ids: vec![],
            }
        });
        if !info.test_case_ids.contains(&tc.id) {
            info.test_case_ids.push(tc.id);
            info.test_case_ids.sort_unstable();
        }
    }

    pub fn get(&self, key: &str) -> CaseInfo {
        self.by_key[key].clone()
    }
}

/// Sort key of a case for stable output.
pub fn case_sort_key(c: &CaseInfo) -> (String, i32, String) {
    (
        c.scenario.clone(),
        c.scenario_version,
        canonical(&c.parameters),
    )
}

// Evaluation ----------------------------------------------------------------------------

/// Evaluates a scalar metric on samples, chunk by chunk. For each sample
/// the result is `None` if it is excluded (missing input or error), else
/// the value or no value. `progress` is called with the number of samples
/// done after each chunk.
pub async fn eval_scalars(
    pool: &PgPool,
    prepared: &Prepared,
    samples: &[SampleRow],
    excluded: &mut Excluded,
    progress: &mut (dyn FnMut(usize) + Send),
) -> ApiResult<Vec<Option<Option<f64>>>> {
    if prepared.signature.kind != Kind::Scalar {
        return Err(ApiError::invalid(format!(
            "{} is a series; this view needs a scalar metric, e.g. max({})",
            prepared.expression, prepared.expression
        )));
    }
    let mut evaluator = prepared.evaluator();
    let mut out = Vec::with_capacity(samples.len());
    let mut done = 0;
    for chunk in samples.chunks(CHUNK) {
        let ids: Vec<Uuid> = chunk.iter().map(|s| s.sample_id).collect();
        let data = derived::load_sample_data(pool, &ids, evaluator.inputs()).await?;
        for s in chunk {
            out.push(match evaluator.evaluate(&data[&s.sample_id]) {
                Ok(Outcome {
                    value: DslValue::Scalar(v),
                    ..
                }) => Some(v),
                Ok(_) => None,
                Err(e) => {
                    excluded.record(&e);
                    None
                }
            });
        }
        done += chunk.len();
        progress(done);
        tokio::task::yield_now().await;
    }
    Ok(out)
}

// Aggregates (FR-12) ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AggregatesRequest {
    /// Run filter (FR-11). `limit` and `cursor` do not apply.
    #[serde(default)]
    pub filter: RunFilter,
    /// The metric: an uploaded scalar metric (`load_time`), a derived metric
    /// (`derived.throughput`) or any scalar DSL expression.
    pub metric: String,
    /// Convert values to this unit (FR-13).
    pub unit: Option<String>,
    /// Include warmup samples (FR-12; excluded by default).
    #[serde(default)]
    pub include_warmup: bool,
    /// Percentiles in [0, 100]; default 5, 25, 75, 95.
    pub percentiles: Option<Vec<f64>>,
    /// Level of the confidence interval; default 0.95.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AggregateGroup {
    pub run: RunRef,
    pub case: CaseInfo,
    /// Device name after aliases.
    pub client_device: String,
    pub case_execution_ids: Vec<Uuid>,
    pub stats: Stats,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AggregatesResponse {
    pub metric: MetricInfo,
    /// One group per run, case and client device, in run order.
    pub groups: Vec<AggregateGroup>,
    pub excluded: Excluded,
}

/// Values grouped by run, case key and device, in run order.
pub struct Grouped {
    pub cases: Cases,
    /// (run, case key, device) -> (case executions, values)
    pub groups: Vec<((Uuid, String, String), (Vec<Uuid>, Vec<Option<f64>>))>,
}

pub fn group_values(ds: &Dataset, values: &[Option<Option<f64>>], drop: &[String]) -> Grouped {
    let mut cases = Cases::default();
    let mut index: HashMap<(Uuid, String, String), usize> = HashMap::new();
    let mut groups: Vec<((Uuid, String, String), (Vec<Uuid>, Vec<Option<f64>>))> = Vec::new();
    for (s, v) in ds.samples.iter().zip(values) {
        let Some(v) = v else { continue };
        let tc = &ds.test_cases[&s.test_case_id];
        let key = case_key(tc, drop);
        cases.add(&key, tc, drop);
        let k = (s.run_id, key, s.client_device.clone());
        let i = *index.entry(k.clone()).or_insert_with(|| {
            groups.push((k, (vec![], vec![])));
            groups.len() - 1
        });
        let (ces, vals) = &mut groups[i].1;
        if !ces.contains(&s.case_execution_id) {
            ces.push(s.case_execution_id);
        }
        vals.push(*v);
    }
    Grouped { cases, groups }
}

pub async fn aggregates(pool: &PgPool, req: &AggregatesRequest) -> ApiResult<AggregatesResponse> {
    let options = StatsOptions::new(&req.percentiles, req.confidence)?;
    let prepared = derived::prepare(pool, &req.metric, req.unit.as_deref()).await?;
    let aliases = Aliases::load(pool).await?;
    let ds = load_dataset(
        pool,
        &req.filter,
        req.include_warmup,
        &Restrict::default(),
        &aliases,
    )
    .await?;
    let mut excluded = Excluded::default();
    let values = eval_scalars(pool, &prepared, &ds.samples, &mut excluded, &mut |_| {}).await?;
    let grouped = group_values(&ds, &values, &[]);
    let mut groups: Vec<AggregateGroup> = grouped
        .groups
        .into_iter()
        .map(|((run, key, device), (ces, vals))| AggregateGroup {
            run: ds.runs[&run].clone(),
            case: grouped.cases.get(&key),
            client_device: device,
            case_execution_ids: ces,
            stats: options.summarize(&vals),
        })
        .collect();
    groups.sort_by(|a, b| {
        (
            a.run.started_at,
            a.run.id,
            case_sort_key(&a.case),
            &a.client_device,
        )
            .cmp(&(
                b.run.started_at,
                b.run.id,
                case_sort_key(&b.case),
                &b.client_device,
            ))
    });
    Ok(AggregatesResponse {
        metric: MetricInfo::of(&prepared),
        groups,
        excluded,
    })
}

// Trend (FR-18) ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrendOrder {
    /// By measured start time.
    #[default]
    StartedAt,
    /// Grouped by SUT version (in order of their first run), then by start
    /// time.
    SutVersion,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TrendRequest {
    /// Any test case of the case. Test cases whose parameters are equal
    /// after aliases belong to the same case; other scenario versions never
    /// do.
    pub test_case_id: i64,
    /// Client device name after aliases.
    pub client_device: String,
    pub metric: String,
    pub unit: Option<String>,
    /// Further restrictions, e.g. a time range or a branch (FR-11).
    #[serde(default)]
    pub filter: RunFilter,
    #[serde(default)]
    pub include_warmup: bool,
    #[serde(default)]
    pub order_by: TrendOrder,
    /// Normalize to the mean of this run (FR-13).
    pub reference_run_id: Option<Uuid>,
    pub percentiles: Option<Vec<f64>>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrendMarker {
    /// The device details (OS, driver, ...) differ from the previous point.
    DeviceDetailsChanged,
    /// The shared environment differs from the previous point.
    SharedEnvironmentChanged,
    /// The SUT version differs from the previous point.
    SutVersionChanged,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrendPoint {
    pub run: RunRef,
    pub stats: Stats,
    /// Mean divided by the reference run's mean, if a reference is set.
    pub relative_to_reference: Option<f64>,
    pub device_environment_ids: Vec<i64>,
    pub markers: Vec<TrendMarker>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrendResponse {
    pub metric: MetricInfo,
    pub case: CaseInfo,
    pub client_device: String,
    pub points: Vec<TrendPoint>,
    /// Annotations on the runs, on the case's test cases, and points in time
    /// within the trend.
    pub annotations: Vec<Annotation>,
    pub excluded: Excluded,
}

pub async fn trend(pool: &PgPool, req: &TrendRequest) -> ApiResult<TrendResponse> {
    let options = StatsOptions::new(&req.percentiles, req.confidence)?;
    let prepared = derived::prepare(pool, &req.metric, req.unit.as_deref()).await?;
    let aliases = Aliases::load(pool).await?;
    let anchor = load_test_cases(pool, &[req.test_case_id], &aliases)
        .await?
        .remove(&req.test_case_id)
        .ok_or_else(|| ApiError::not_found(format!("test case {} not found", req.test_case_id)))?;
    let key = case_key(&anchor, &[]);
    let device = aliases.resolve_device(&req.client_device);
    let restrict = Restrict {
        scenario_version_id: Some(anchor.scenario_version_id),
        device_names: Some(aliases.raw_device_names(&device)),
    };
    let mut ds = load_dataset(pool, &req.filter, req.include_warmup, &restrict, &aliases).await?;
    ds.samples
        .retain(|s| case_key(&ds.test_cases[&s.test_case_id], &[]) == key);

    let mut excluded = Excluded::default();
    let values = eval_scalars(pool, &prepared, &ds.samples, &mut excluded, &mut |_| {}).await?;
    let grouped = group_values(&ds, &values, &[]);
    let mut env_ids: HashMap<Uuid, Vec<i64>> = HashMap::new();
    for s in &ds.samples {
        let ids = env_ids.entry(s.run_id).or_default();
        if !ids.contains(&s.device_environment_id) {
            ids.push(s.device_environment_id);
            ids.sort_unstable();
        }
    }
    let mut case = CaseInfo {
        test_case_ids: vec![],
        ..grouped.cases.by_key.get(&key).cloned().unwrap_or_else(|| {
            let mut c = Cases::default();
            c.add(&key, &anchor, &[]);
            c.get(&key)
        })
    };
    let mut points: Vec<TrendPoint> = Vec::new();
    for ((run, _, _), (_, vals)) in grouped.groups {
        points.push(TrendPoint {
            run: ds.runs[&run].clone(),
            stats: options.summarize(&vals),
            relative_to_reference: None,
            device_environment_ids: env_ids.remove(&run).unwrap_or_default(),
            markers: vec![],
        });
    }
    for s in &ds.samples {
        if !case.test_case_ids.contains(&s.test_case_id) {
            case.test_case_ids.push(s.test_case_id);
        }
    }
    if case.test_case_ids.is_empty() {
        case.test_case_ids.push(anchor.id);
    }
    case.test_case_ids.sort_unstable();

    match req.order_by {
        TrendOrder::StartedAt => points.sort_by_key(|p| (p.run.started_at, p.run.id)),
        TrendOrder::SutVersion => {
            let mut first: HashMap<i64, DateTime<Utc>> = HashMap::new();
            for p in &points {
                let e = first
                    .entry(p.run.sut_version.id)
                    .or_insert(p.run.started_at);
                *e = (*e).min(p.run.started_at);
            }
            points.sort_by_key(|p| {
                (
                    first[&p.run.sut_version.id],
                    p.run.sut_version.id,
                    p.run.started_at,
                    p.run.id,
                )
            });
        }
    }
    for i in 1..points.len() {
        let (prev, cur) = (&points[i - 1], &points[i]);
        let mut markers = vec![];
        if prev.device_environment_ids != cur.device_environment_ids {
            markers.push(TrendMarker::DeviceDetailsChanged);
        }
        if prev.run.shared_environment_id != cur.run.shared_environment_id {
            markers.push(TrendMarker::SharedEnvironmentChanged);
        }
        if prev.run.sut_version.id != cur.run.sut_version.id {
            markers.push(TrendMarker::SutVersionChanged);
        }
        points[i].markers = markers;
    }
    if let Some(reference) = req.reference_run_id {
        let ref_mean = points
            .iter()
            .find(|p| p.run.id == reference)
            .ok_or_else(|| {
                ApiError::invalid(format!("reference run {reference} is not in the trend"))
            })?
            .stats
            .mean;
        for p in &mut points {
            p.relative_to_reference = match (p.stats.mean, ref_mean) {
                (Some(m), Some(r)) if r != 0.0 => Some(m / r),
                _ => None,
            };
        }
    }

    let run_ids: Vec<Uuid> = points.iter().map(|p| p.run.id).collect();
    let (from, to) = match (
        points.iter().map(|p| p.run.started_at).min(),
        points.iter().map(|p| p.run.started_at).max(),
    ) {
        (Some(a), Some(b)) => (Some(a), Some(b)),
        _ => (None, None),
    };
    let annotations: Vec<Annotation> = sqlx::query_as(
        "SELECT id, run_id, test_case_id, occurred_at, body, created_at, updated_at FROM annotation \
         WHERE run_id = ANY($1) OR test_case_id = ANY($2) \
            OR (occurred_at >= $3 AND occurred_at <= $4) \
         ORDER BY coalesce(occurred_at, created_at), id",
    )
    .bind(&run_ids)
    .bind(&case.test_case_ids)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    Ok(TrendResponse {
        metric: MetricInfo::of(&prepared),
        case,
        client_device: device,
        points,
        annotations,
        excluded,
    })
}

/// Dimension values compare and sort by their canonical form.
pub fn value_key(v: &Value) -> String {
    canonical(v)
}

/// Resolves a parameter value through aliases (used for values given in
/// requests).
pub fn resolve_parameter(aliases: &Aliases, name: &str, value: &Value) -> Value {
    aliases.resolve(&parameter_dimension(name), value)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn canonical_form_ignores_key_order_and_number_format() {
        assert_eq!(
            canonical(&json!({"b": 1, "a": "x"})),
            canonical(&json!({"a": "x", "b": 1.0}))
        );
        assert_ne!(canonical(&json!({"a": 1})), canonical(&json!({"a": "1"})));
        assert_ne!(
            canonical(&json!({"a": true})),
            canonical(&json!({"a": "true"}))
        );
    }

    fn tc(id: i64, params: Value) -> TestCaseRow {
        TestCaseRow {
            id,
            scenario: "s".into(),
            scenario_version: 1,
            scenario_version_id: 7,
            parameters: params.as_object().unwrap().clone(),
            parameter_units: serde_json::Map::new(),
            assets: vec![],
        }
    }

    #[test]
    fn case_keys_drop_parameters() {
        let a = tc(1, json!({"pipeline": "new", "size": 10}));
        let b = tc(2, json!({"pipeline": "classic", "size": 10}));
        assert_ne!(case_key(&a, &[]), case_key(&b, &[]));
        assert_eq!(
            case_key(&a, &["pipeline".into()]),
            case_key(&b, &["pipeline".into()])
        );
        let mut cases = Cases::default();
        let k = case_key(&a, &["pipeline".into()]);
        cases.add(&k, &a, &["pipeline".into()]);
        cases.add(&k, &b, &["pipeline".into()]);
        let info = cases.get(&k);
        assert_eq!(info.parameters, json!({"size": 10}));
        assert_eq!(info.test_case_ids, vec![1, 2]);
    }

    #[test]
    fn stats_options_validate() {
        assert!(StatsOptions::new(&Some(vec![101.0]), 0.95).is_err());
        assert!(StatsOptions::new(&None, 1.0).is_err());
        let o = StatsOptions::new(&None, 0.9).unwrap();
        assert_eq!(o.percentiles, DEFAULT_PERCENTILES.to_vec());
    }
}
