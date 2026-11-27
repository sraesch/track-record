//! Incremental, idempotent ingestion (FR-1 to FR-5).
//!
//! A run is created, case executions, samples and series are added as they
//! finish, and the run is closed. Every result has a client-generated UUID.
//! Sending the same content again is a no-op; different content under an
//! existing id is a conflict, because results are immutable (FR-3, FR-6).
//!
//! Definitions and dimensions are created only through the `ensure_*`
//! functions of the schema. Each request runs in one transaction, so a
//! rejected request leaves nothing behind, not even new definitions.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, SubsecRound, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::{MetricDirection, MetricValueType, RunStatus, RunTrigger};
use crate::error::{ApiError, ApiResult};
use crate::series_store::{self, InsertOutcome, NewSeries, Points};

// Request bodies --------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedDefinition {
    pub name: String,
    /// Free-form JSON definition. Identical definitions map to the same
    /// version; a changed definition is a new version.
    pub definition: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SutVersionInput {
    pub commit: String,
    pub branch: Option<String>,
    pub build_config: Option<String>,
    pub tag: Option<String>,
}

/// Creates a run (FR-1). The run starts as `running`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRun {
    pub suite: NamedDefinition,
    pub sut_version: SutVersionInput,
    /// Details of the shared environment (server, backend, ...). Identical
    /// details are stored once (fingerprint).
    #[schema(value_type = Object)]
    pub shared_environment: Map<String, Value>,
    pub triggered_by: RunTrigger,
    /// Measured start time from the harness. Trends and time filters use it.
    pub started_at: DateTime<Utc>,
    /// Tags the run is created with.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetInput {
    pub name: String,
    pub version: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TestCaseInput {
    pub scenario: NamedDefinition,
    #[serde(default)]
    pub assets: Vec<AssetInput>,
    /// Flat object of numbers, strings and booleans. Types are kept. Names
    /// cannot contain a backtick (`` ` ``), which expressions use to quote
    /// names.
    #[serde(default)]
    #[schema(value_type = Object)]
    pub parameters: Map<String, Value>,
    /// Units of numeric parameters, e.g. `{"bandwidth": "Mbit/s"}` (FR-13a).
    #[serde(default)]
    pub parameter_units: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientDeviceInput {
    /// Stable name of the device, e.g. `pixel-8-lab-1`.
    pub name: String,
    /// Device details (model, OS, driver, build). A change is marked in
    /// trends but does not split them.
    #[schema(value_type = Object)]
    pub details: Map<String, Value>,
}

/// Definition of a metric used in the request. It creates the metric if
/// there is none with this name. A metric that exists is not changed: its
/// direction and description stay as they are, and the unit and value type
/// must be the ones it has (otherwise 409, `metric_definition_conflict`).
/// Metrics are changed in the web UI.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricInput {
    /// Unit symbol; `""` for dimensionless (FR-13a).
    pub unit: String,
    pub value_type: MetricValueType,
    pub direction: MetricDirection,
    #[serde(default)]
    pub description: String,
}

/// A measured value: a number, or a boolean for boolean metrics.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum MeasurementValue {
    Number(f64),
    Bool(bool),
}

/// A raw, measured series: one per metric and sample. x never decreases.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SeriesInput {
    pub id: Uuid,
    /// Metric name; must be defined in `metrics`.
    pub metric: String,
    /// What x measures, e.g. `time` or `transferred`.
    pub x_dimension: String,
    /// Unit of x; `""` for dimensionless.
    pub x_unit: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Debug, Clone)]
struct SeriesData {
    metric: String,
    x_dimension: String,
    x_unit: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SampleInput {
    pub id: Uuid,
    pub repetition: i32,
    #[serde(default)]
    pub is_warmup: bool,
    /// Metric name -> value; every metric must be defined in `metrics`.
    #[serde(default)]
    pub measurements: BTreeMap<String, MeasurementValue>,
    #[serde(default)]
    pub series: Vec<SeriesInput>,
}

/// Adds a case execution to a running run, or a correction (`supersedes_id`)
/// to any run (FR-6). Samples may be sent inline or added later.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCaseExecution {
    pub test_case: TestCaseInput,
    pub client_device: ClientDeviceInput,
    /// Case execution of the same run, test case and device this one
    /// corrects.
    pub supersedes_id: Option<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Definitions of the metrics used in `samples`.
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricInput>,
    #[serde(default)]
    pub samples: Vec<SampleInput>,
}

/// Adds one sample to a case execution.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSample {
    pub repetition: i32,
    #[serde(default)]
    pub is_warmup: bool,
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricInput>,
    #[serde(default)]
    pub measurements: BTreeMap<String, MeasurementValue>,
    #[serde(default)]
    pub series: Vec<SeriesInput>,
}

/// Adds one series to a sample.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSeries {
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricInput>,
    pub metric: String,
    pub x_dimension: String,
    pub x_unit: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CloseStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CloseRun {
    pub status: CloseStatus,
    /// Measured end time; defaults to now.
    pub ended_at: Option<DateTime<Utc>>,
}

/// Result of an idempotent write.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IngestResponse {
    pub id: Uuid,
    /// False if everything already existed with identical content (a retry).
    pub created: bool,
}

// Validation -------------------------------------------------------------------

const MAX_SERIES_POINTS: usize = 1_000_000;

fn require_name(field: &str, value: &str) -> ApiResult<()> {
    if value.trim().is_empty() {
        return Err(ApiError::invalid(format!("{field} must not be empty")));
    }
    Ok(())
}

fn normalize_tags(tags: &[String]) -> ApiResult<Vec<String>> {
    let mut out = BTreeSet::new();
    for tag in tags {
        let tag = tag.trim();
        require_name("tag", tag)?;
        out.insert(tag.to_owned());
    }
    Ok(out.into_iter().collect())
}

/// Timestamps are stored with microsecond precision; comparing retries needs
/// the same precision.
fn micros(t: DateTime<Utc>) -> DateTime<Utc> {
    t.trunc_subsecs(6)
}

/// Chart expressions read parameters by name, quoting names with other
/// characters in backticks (``param.`frame size` ``); a name with a backtick
/// could not be written there (ADR 0022). Checked only for new case
/// executions: earlier releases stored such names, and an identical retry
/// of one stays a 200 (FR-3).
fn check_parameter_names(tc: &TestCaseInput) -> ApiResult<()> {
    match tc.parameters.keys().find(|name| name.contains('`')) {
        Some(name) => Err(ApiError::invalid(format!(
            "parameter {name:?}: a parameter name cannot contain a backtick (`)"
        ))),
        None => Ok(()),
    }
}

fn validate_test_case(tc: &TestCaseInput) -> ApiResult<()> {
    require_name("test_case.scenario.name", &tc.scenario.name)?;
    for (name, value) in &tc.parameters {
        if !(value.is_number() || value.is_string() || value.is_boolean()) {
            return Err(ApiError::invalid(format!(
                "parameter {name:?} must be a number, string or boolean"
            )));
        }
    }
    for name in tc.parameter_units.keys() {
        match tc.parameters.get(name) {
            Some(v) if v.is_number() => {}
            _ => {
                return Err(ApiError::invalid(format!(
                    "parameter_units: {name:?} is not a numeric parameter"
                )));
            }
        }
    }
    for asset in &tc.assets {
        require_name("asset name", &asset.name)?;
        if asset.version.is_none() && asset.content_hash.is_none() {
            return Err(ApiError::invalid(format!(
                "asset {:?} needs a version or a content_hash",
                asset.name
            )));
        }
    }
    Ok(())
}

fn validate_series(data: &SeriesData) -> ApiResult<Points> {
    require_name("series.x_dimension", &data.x_dimension)?;
    if data.x.len() != data.y.len() {
        return Err(ApiError::invalid(format!(
            "series of {:?}: x has {} values, y has {}",
            data.metric,
            data.x.len(),
            data.y.len()
        )));
    }
    if data.x.len() > MAX_SERIES_POINTS {
        return Err(ApiError::invalid(format!(
            "a series has at most {MAX_SERIES_POINTS} points"
        )));
    }
    // Stored as f32 (ADR 0002); a value that overflows f32 is not finite.
    let to_f32 = |values: &[f64], axis: &str| -> ApiResult<Vec<f32>> {
        values
            .iter()
            .map(|v| {
                let f = *v as f32;
                if f.is_finite() {
                    Ok(f)
                } else {
                    Err(ApiError::invalid(format!(
                        "series of {:?}: {axis} value {v} is not a finite 32-bit float",
                        data.metric
                    )))
                }
            })
            .collect()
    };
    let points = Points {
        x: to_f32(&data.x, "x")?,
        y: to_f32(&data.y, "y")?,
    };
    if points.x.windows(2).any(|w| w[1] < w[0]) {
        return Err(ApiError::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "series_x_decreasing",
            format!("series of {:?}: x values must not decrease", data.metric),
        ));
    }
    Ok(points)
}

fn measurement_value(
    metric: &str,
    value_type: MetricValueType,
    value: MeasurementValue,
) -> ApiResult<f64> {
    let v = match (value_type, value) {
        (MetricValueType::Boolean, MeasurementValue::Bool(b)) => f64::from(u8::from(b)),
        (MetricValueType::Boolean, MeasurementValue::Number(n)) if n == 0.0 || n == 1.0 => n,
        (MetricValueType::Integer, MeasurementValue::Number(n)) if n.fract() == 0.0 => n,
        (MetricValueType::Float, MeasurementValue::Number(n)) => n,
        (t, _) => {
            return Err(ApiError::invalid(format!(
                "measurement {metric:?}: value does not match value type {t:?}"
            )));
        }
    };
    if !v.is_finite() {
        return Err(ApiError::invalid(format!(
            "measurement {metric:?} must be finite"
        )));
    }
    Ok(v)
}

// Definitions and dimensions -----------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct MetricRef {
    id: i64,
    value_type: MetricValueType,
}

/// Maps the metric names of a request to the metrics, creating those that
/// do not exist yet. An upload never changes a metric (ADR 0018): the
/// direction and description of an existing one stay as they are, and
/// another unit or value type is rejected (409,
/// `metric_definition_conflict`), as the values would be stored under the
/// wrong unit.
async fn ensure_metrics(
    conn: &mut PgConnection,
    metrics: &BTreeMap<String, MetricInput>,
) -> ApiResult<HashMap<String, MetricRef>> {
    let mut out = HashMap::new();
    for (name, m) in metrics {
        require_name("metric name", name)?;
        let id: i64 = sqlx::query_scalar("SELECT ensure_metric($1, $2, $3, $4, $5)")
            .bind(name)
            .bind(&m.unit)
            .bind(m.value_type)
            .bind(m.direction)
            .bind(&m.description)
            .fetch_one(&mut *conn)
            .await?;
        out.insert(
            name.clone(),
            MetricRef {
                id,
                value_type: m.value_type,
            },
        );
    }
    Ok(out)
}

fn metric<'a>(metrics: &'a HashMap<String, MetricRef>, name: &str) -> ApiResult<&'a MetricRef> {
    metrics.get(name).ok_or_else(|| {
        ApiError::invalid(format!(
            "metric {name:?} is used but not defined in `metrics`"
        ))
    })
}

async fn ensure_test_case(conn: &mut PgConnection, tc: &TestCaseInput) -> ApiResult<i64> {
    let scenario_version_id: i64 = sqlx::query_scalar("SELECT ensure_scenario_version($1, $2)")
        .bind(&tc.scenario.name)
        .bind(&tc.scenario.definition)
        .fetch_one(&mut *conn)
        .await?;
    let mut asset_ids = Vec::with_capacity(tc.assets.len());
    for asset in &tc.assets {
        let id: i64 = sqlx::query_scalar("SELECT ensure_asset($1, $2, $3)")
            .bind(&asset.name)
            .bind(&asset.version)
            .bind(&asset.content_hash)
            .fetch_one(&mut *conn)
            .await?;
        asset_ids.push(id);
    }
    let units: Map<String, Value> = tc
        .parameter_units
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    Ok(
        sqlx::query_scalar("SELECT ensure_test_case($1, $2, $3, $4)")
            .bind(scenario_version_id)
            .bind(&asset_ids)
            .bind(Value::Object(tc.parameters.clone()))
            .bind(Value::Object(units))
            .fetch_one(&mut *conn)
            .await?,
    )
}

async fn ensure_environment(
    conn: &mut PgConnection,
    details: &Map<String, Value>,
) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT ensure_environment($1)")
        .bind(Value::Object(details.clone()))
        .fetch_one(&mut *conn)
        .await?)
}

// Runs ----------------------------------------------------------------------------

#[derive(sqlx::FromRow, PartialEq, Debug)]
struct RunIdentity {
    suite_version_id: i64,
    sut_version_id: i64,
    shared_environment_id: i64,
    triggered_by: RunTrigger,
    started_at: DateTime<Utc>,
}

pub async fn create_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    req: &CreateRun,
) -> ApiResult<bool> {
    require_name("suite.name", &req.suite.name)?;
    require_name("sut_version.commit", &req.sut_version.commit)?;
    let tags = normalize_tags(&req.tags)?;

    let suite_version_id: i64 = sqlx::query_scalar("SELECT ensure_suite_version($1, $2)")
        .bind(&req.suite.name)
        .bind(&req.suite.definition)
        .fetch_one(&mut **tx)
        .await?;
    let sut = &req.sut_version;
    let sut_version_id: i64 = sqlx::query_scalar("SELECT ensure_sut_version($1, $2, $3, $4)")
        .bind(&sut.commit)
        .bind(&sut.branch)
        .bind(&sut.build_config)
        .bind(&sut.tag)
        .fetch_one(&mut **tx)
        .await?;
    let shared_environment_id = ensure_environment(tx, &req.shared_environment).await?;
    let wanted = RunIdentity {
        suite_version_id,
        sut_version_id,
        shared_environment_id,
        triggered_by: req.triggered_by,
        started_at: micros(req.started_at),
    };

    let inserted = sqlx::query(
        "INSERT INTO run (id, suite_version_id, sut_version_id, shared_environment_id, \
                          triggered_by, started_at) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
    )
    .bind(run_id)
    .bind(wanted.suite_version_id)
    .bind(wanted.sut_version_id)
    .bind(wanted.shared_environment_id)
    .bind(wanted.triggered_by)
    .bind(wanted.started_at)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1;

    if inserted {
        for tag in &tags {
            sqlx::query("INSERT INTO run_tag (run_id, tag) VALUES ($1, $2)")
                .bind(run_id)
                .bind(tag)
                .execute(&mut **tx)
                .await?;
        }
        return Ok(true);
    }

    // Retry: tags may have been edited since, so only the immutable parts
    // are compared.
    let stored: RunIdentity = sqlx::query_as(
        "SELECT suite_version_id, sut_version_id, shared_environment_id, triggered_by, started_at \
         FROM run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;
    if stored != wanted {
        return Err(ApiError::conflict(format!(
            "run {run_id} already exists with different content"
        )));
    }
    Ok(false)
}

/// Closes a run (FR-2). Sending the same terminal status again is a no-op.
pub async fn close_run(conn: &mut PgConnection, run_id: Uuid, req: &CloseRun) -> ApiResult<bool> {
    let status = match req.status {
        CloseStatus::Completed => RunStatus::Completed,
        CloseStatus::Failed => RunStatus::Failed,
    };
    let updated = sqlx::query(
        "UPDATE run SET status = $2, ended_at = $3 WHERE id = $1 AND status = 'running'",
    )
    .bind(run_id)
    .bind(status)
    .bind(req.ended_at.map(micros))
    .execute(&mut *conn)
    .await?
    .rows_affected()
        == 1;
    if updated {
        return Ok(true);
    }
    let current: Option<RunStatus> = sqlx::query_scalar("SELECT status FROM run WHERE id = $1")
        .bind(run_id)
        .fetch_optional(&mut *conn)
        .await?;
    match current {
        None => Err(ApiError::not_found(format!("run {run_id} not found"))),
        Some(s) if s == status => Ok(false),
        Some(s) => Err(ApiError::new(
            axum::http::StatusCode::CONFLICT,
            "invalid_status_transition",
            format!("run {run_id} is already {s:?}"),
        )),
    }
}

// Case executions, samples and series -------------------------------------------

async fn require_run(conn: &mut PgConnection, run_id: Uuid) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM run WHERE id = $1)")
        .bind(run_id)
        .fetch_one(&mut *conn)
        .await?;
    if !exists {
        return Err(ApiError::not_found(format!("run {run_id} not found")));
    }
    Ok(())
}

/// Checks that a case execution exists and belongs to the run.
async fn require_case_execution(
    conn: &mut PgConnection,
    run_id: Uuid,
    ce_id: Uuid,
) -> ApiResult<()> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT run_id FROM case_execution WHERE id = $1")
        .bind(ce_id)
        .fetch_optional(&mut *conn)
        .await?;
    match owner {
        Some(r) if r == run_id => Ok(()),
        _ => Err(ApiError::not_found(format!(
            "case execution {ce_id} not found in run {run_id}"
        ))),
    }
}

#[derive(sqlx::FromRow, PartialEq, Debug)]
struct CaseExecutionIdentity {
    run_id: Uuid,
    test_case_id: i64,
    client_device_id: i64,
    device_environment_id: i64,
    supersedes_id: Option<Uuid>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
}

pub async fn create_case_execution(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    ce_id: Uuid,
    req: &CreateCaseExecution,
) -> ApiResult<bool> {
    validate_test_case(&req.test_case)?;
    require_name("client_device.name", &req.client_device.name)?;
    if let (Some(s), Some(e)) = (req.started_at, req.ended_at)
        && e < s
    {
        return Err(ApiError::invalid("ended_at must not be before started_at"));
    }
    require_run(tx, run_id).await?;

    let test_case_id = ensure_test_case(tx, &req.test_case).await?;
    let client_device_id: i64 = sqlx::query_scalar("SELECT ensure_client_device($1)")
        .bind(&req.client_device.name)
        .fetch_one(&mut **tx)
        .await?;
    let device_environment_id = ensure_environment(tx, &req.client_device.details).await?;
    let wanted = CaseExecutionIdentity {
        run_id,
        test_case_id,
        client_device_id,
        device_environment_id,
        supersedes_id: req.supersedes_id,
        started_at: req.started_at.map(micros),
        ended_at: req.ended_at.map(micros),
    };

    if let Some(superseded) = req.supersedes_id {
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM case_execution WHERE id = $1 AND run_id = $2 \
                            AND test_case_id = $3 AND client_device_id = $4)",
        )
        .bind(superseded)
        .bind(run_id)
        .bind(test_case_id)
        .bind(client_device_id)
        .fetch_one(&mut **tx)
        .await?;
        if !ok {
            return Err(ApiError::invalid(format!(
                "supersedes_id {superseded} is not a case execution of the same run, test case and device"
            )));
        }
    }

    // A retry of a stored case execution must not hit the "run is not
    // running" trigger, which fires before the conflict check: the run may
    // have been closed since.
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM case_execution WHERE id = $1)")
            .bind(ce_id)
            .fetch_one(&mut **tx)
            .await?;
    if !exists {
        check_parameter_names(&req.test_case)?;
    }
    let inserted = !exists
        && sqlx::query(
            "INSERT INTO case_execution (id, run_id, test_case_id, client_device_id, \
                                     device_environment_id, supersedes_id, started_at, ended_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (id) DO NOTHING",
        )
        .bind(ce_id)
        .bind(wanted.run_id)
        .bind(wanted.test_case_id)
        .bind(wanted.client_device_id)
        .bind(wanted.device_environment_id)
        .bind(wanted.supersedes_id)
        .bind(wanted.started_at)
        .bind(wanted.ended_at)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1;

    if !inserted {
        let stored: CaseExecutionIdentity = sqlx::query_as(
            "SELECT run_id, test_case_id, client_device_id, device_environment_id, supersedes_id, \
                    started_at, ended_at FROM case_execution WHERE id = $1",
        )
        .bind(ce_id)
        .fetch_one(&mut **tx)
        .await?;
        if stored != wanted {
            return Err(ApiError::conflict(format!(
                "case execution {ce_id} already exists with different content"
            )));
        }
    }

    let metrics = ensure_metrics(tx, &req.metrics).await?;
    let mut created = inserted;
    for sample in &req.samples {
        let data = SampleData {
            repetition: sample.repetition,
            is_warmup: sample.is_warmup,
            measurements: &sample.measurements,
            series: &sample.series,
        };
        created |= insert_sample(tx, ce_id, sample.id, &data, &metrics).await?;
    }
    Ok(created)
}

struct SampleData<'a> {
    repetition: i32,
    is_warmup: bool,
    measurements: &'a BTreeMap<String, MeasurementValue>,
    series: &'a [SeriesInput],
}

async fn insert_sample(
    conn: &mut PgConnection,
    ce_id: Uuid,
    sample_id: Uuid,
    data: &SampleData<'_>,
    metrics: &HashMap<String, MetricRef>,
) -> ApiResult<bool> {
    if data.repetition < 0 {
        return Err(ApiError::invalid("repetition must not be negative"));
    }
    let mut values: BTreeMap<i64, f64> = BTreeMap::new();
    for (name, value) in data.measurements {
        let m = metric(metrics, name)?;
        values.insert(m.id, measurement_value(name, m.value_type, *value)?);
    }

    let inserted = sqlx::query(
        "INSERT INTO sample (id, case_execution_id, repetition, is_warmup) \
         VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
    )
    .bind(sample_id)
    .bind(ce_id)
    .bind(data.repetition)
    .bind(data.is_warmup)
    .execute(&mut *conn)
    .await?
    .rows_affected()
        == 1;

    if inserted {
        let (ids, vals): (Vec<i64>, Vec<f64>) = values.iter().map(|(k, v)| (*k, *v)).unzip();
        sqlx::query(
            "INSERT INTO measurement (sample_id, metric_id, value) \
             SELECT $1, m, v FROM unnest($2::bigint[], $3::float8[]) AS t(m, v)",
        )
        .bind(sample_id)
        .bind(&ids)
        .bind(&vals)
        .execute(&mut *conn)
        .await?;
    } else {
        let stored: (Uuid, i32, bool) = sqlx::query_as(
            "SELECT case_execution_id, repetition, is_warmup FROM sample WHERE id = $1",
        )
        .bind(sample_id)
        .fetch_one(&mut *conn)
        .await?;
        let stored_values: Vec<(i64, f64)> =
            sqlx::query_as("SELECT metric_id, value FROM measurement WHERE sample_id = $1")
                .bind(sample_id)
                .fetch_all(&mut *conn)
                .await?;
        let stored_values: BTreeMap<i64, f64> = stored_values.into_iter().collect();
        if stored != (ce_id, data.repetition, data.is_warmup) || stored_values != values {
            return Err(ApiError::conflict(format!(
                "sample {sample_id} already exists with different content"
            )));
        }
    }

    let mut created = inserted;
    for s in data.series {
        let series = SeriesData {
            metric: s.metric.clone(),
            x_dimension: s.x_dimension.clone(),
            x_unit: s.x_unit.clone(),
            x: s.x.clone(),
            y: s.y.clone(),
        };
        created |= insert_series(conn, sample_id, s.id, &series, metrics).await?;
    }
    Ok(created)
}

async fn insert_series(
    conn: &mut PgConnection,
    sample_id: Uuid,
    series_id: Uuid,
    data: &SeriesData,
    metrics: &HashMap<String, MetricRef>,
) -> ApiResult<bool> {
    let m = metric(metrics, &data.metric)?;
    let points = validate_series(data)?;
    let outcome = series_store::insert(
        conn,
        &NewSeries {
            id: series_id,
            sample_id,
            metric_id: m.id,
            x_dimension: &data.x_dimension,
            x_unit: &data.x_unit,
            points: &points,
        },
    )
    .await?;
    match outcome {
        InsertOutcome::Created => Ok(true),
        InsertOutcome::Unchanged => Ok(false),
        InsertOutcome::Differs => Err(ApiError::conflict(format!(
            "series {series_id} already exists with different content"
        ))),
    }
}

pub async fn create_sample(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    ce_id: Uuid,
    sample_id: Uuid,
    req: &CreateSample,
) -> ApiResult<bool> {
    require_case_execution(tx, run_id, ce_id).await?;
    let metrics = ensure_metrics(tx, &req.metrics).await?;
    let data = SampleData {
        repetition: req.repetition,
        is_warmup: req.is_warmup,
        measurements: &req.measurements,
        series: &req.series,
    };
    insert_sample(tx, ce_id, sample_id, &data, &metrics).await
}

pub async fn create_series(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    ce_id: Uuid,
    sample_id: Uuid,
    series_id: Uuid,
    req: &CreateSeries,
) -> ApiResult<bool> {
    require_case_execution(tx, run_id, ce_id).await?;
    let owner: Option<Uuid> =
        sqlx::query_scalar("SELECT case_execution_id FROM sample WHERE id = $1")
            .bind(sample_id)
            .fetch_optional(&mut **tx)
            .await?;
    if owner != Some(ce_id) {
        return Err(ApiError::not_found(format!(
            "sample {sample_id} not found in case execution {ce_id}"
        )));
    }
    let metrics = ensure_metrics(tx, &req.metrics).await?;
    let data = SeriesData {
        metric: req.metric.clone(),
        x_dimension: req.x_dimension.clone(),
        x_unit: req.x_unit.clone(),
        x: req.x.clone(),
        y: req.y.clone(),
    };
    insert_series(tx, sample_id, series_id, &data, &metrics).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(x: Vec<f64>, y: Vec<f64>) -> SeriesData {
        SeriesData {
            metric: "fps".into(),
            x_dimension: "time".into(),
            x_unit: "s".into(),
            x,
            y,
        }
    }

    #[test]
    fn series_validation() {
        assert!(validate_series(&series(vec![0.0, 1.0, 1.0], vec![1.0, 2.0, 3.0])).is_ok());
        let err = validate_series(&series(vec![1.0, 0.0], vec![1.0, 2.0])).unwrap_err();
        assert_eq!(err.code(), "series_x_decreasing");
        assert!(validate_series(&series(vec![0.0], vec![1.0, 2.0])).is_err());
        assert!(validate_series(&series(vec![0.0], vec![1e39])).is_err());
        let mut empty_dim = series(vec![], vec![]);
        empty_dim.x_dimension = " ".into();
        assert!(validate_series(&empty_dim).is_err());
    }

    #[test]
    fn measurement_values_match_value_types() {
        use MeasurementValue::*;
        use MetricValueType::*;
        assert_eq!(measurement_value("m", Float, Number(1.5)).unwrap(), 1.5);
        assert_eq!(measurement_value("m", Integer, Number(3.0)).unwrap(), 3.0);
        assert!(measurement_value("m", Integer, Number(3.5)).is_err());
        assert_eq!(measurement_value("m", Boolean, Bool(true)).unwrap(), 1.0);
        assert_eq!(measurement_value("m", Boolean, Number(0.0)).unwrap(), 0.0);
        assert!(measurement_value("m", Boolean, Number(2.0)).is_err());
        assert!(measurement_value("m", Float, Bool(true)).is_err());
    }

    #[test]
    fn test_case_validation() {
        let tc = |params: Value, units: &[(&str, &str)]| TestCaseInput {
            scenario: NamedDefinition {
                name: "s".into(),
                definition: Value::Null,
            },
            assets: vec![],
            parameters: params.as_object().unwrap().clone(),
            parameter_units: units
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        };
        assert!(
            validate_test_case(&tc(serde_json::json!({"n": 1, "s": "a"}), &[("n", "MB")])).is_ok()
        );
        assert!(validate_test_case(&tc(serde_json::json!({"n": [1]}), &[])).is_err());
        assert!(validate_test_case(&tc(serde_json::json!({"s": "a"}), &[("s", "MB")])).is_err());
        assert!(validate_test_case(&tc(serde_json::json!({}), &[("n", "MB")])).is_err());
        assert!(check_parameter_names(&tc(serde_json::json!({"tick`count": 3}), &[])).is_err());
        assert!(check_parameter_names(&tc(serde_json::json!({"`": "a"}), &[])).is_err());
        // Spaces, dots and an empty name are fine.
        assert!(
            check_parameter_names(&tc(
                serde_json::json!({"frame size": 1, "a.b": 2, "": 3}),
                &[]
            ))
            .is_ok()
        );
    }

    #[test]
    fn tags_are_trimmed_and_deduplicated() {
        let tags = normalize_tags(&[" a".into(), "b".into(), "a ".into()]).unwrap();
        assert_eq!(tags, vec!["a", "b"]);
        assert!(normalize_tags(&[" ".into()]).is_err());
    }
}
