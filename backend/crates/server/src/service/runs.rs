//! Reading runs and their results: run list with filters (FR-11), run
//! detail, case executions, samples, series and attachment metadata.
//!
//! Superseded case executions (FR-6 corrections) are hidden unless asked
//! for. Invalid runs are excluded from the run list by default (FR-9).
//! Device names and parameter values are shown raw and after aliases.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use track_record_core::series::Series;

use crate::db::{RunStatus, RunTrigger};
use crate::error::{ApiError, ApiResult};
use crate::series_store;
use crate::service::annotations::{self, Annotation};
use crate::service::query::{Aliases, parameter_dimension};

// Run list ----------------------------------------------------------------------

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Deserialize,
    Serialize,
    ToSchema,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Validity {
    /// Only valid runs (default).
    #[default]
    Valid,
    /// Only invalid runs.
    Invalid,
    /// Valid and invalid runs.
    All,
}

/// Run filters (FR-11). Filters on case executions (device, scenario, asset,
/// parameters, metric) match runs with at least one current case execution
/// that satisfies all of them. Device and parameter filters use the values
/// after aliases.
//
// The sources of charts take these fields too: a new field also goes into
// `track_record_core::chart::filter::FIELDS` (a test below checks that the
// table names only fields that exist here).
#[derive(Debug, Clone, Default, Deserialize, IntoParams, ToSchema, schemars::JsonSchema)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct RunFilter {
    /// Run id; repeat for several.
    #[serde(default)]
    pub run_id: Vec<Uuid>,
    /// Suite name.
    pub suite: Option<String>,
    pub sut_commit: Option<String>,
    pub sut_branch: Option<String>,
    pub sut_tag: Option<String>,
    pub sut_build_config: Option<String>,
    /// Measured start time at or after this time.
    pub started_after: Option<DateTime<Utc>>,
    /// Measured start time before this time.
    pub started_before: Option<DateTime<Utc>>,
    /// Measured start within the last N days (at least 1), counted from the
    /// server's time at the request.
    pub started_within_days: Option<i64>,
    /// Of the runs that match all other conditions, only the N (at least 1)
    /// with the newest measured start. `limit` and `cursor` page through them.
    pub latest_runs: Option<i64>,
    /// Measured start at or after that of the earliest run with this run tag.
    /// The marked run is looked up among all runs, whatever the other
    /// conditions say; 422 if no run has the tag.
    pub since_tag: Option<String>,
    /// Measured start at or after that of the earliest run whose SUT version
    /// has this tag, e.g. a release. Looked up among all runs, whatever the
    /// other conditions say; 422 if no run has the tag.
    pub since_sut_tag: Option<String>,
    /// Status; repeat for several.
    #[serde(default)]
    pub status: Vec<RunStatus>,
    pub triggered_by: Option<RunTrigger>,
    pub shared_environment_id: Option<i64>,
    /// Client device name after aliases.
    pub client_device: Option<String>,
    /// Scenario name.
    pub scenario: Option<String>,
    /// Asset name.
    pub asset: Option<String>,
    /// Parameter filter `name=value`, repeatable. The value is read as JSON
    /// (`size=10` is a number, `mode="10"` a string); anything that is not a
    /// JSON scalar is a string (`mode=fast`).
    #[serde(default)]
    pub param: Vec<String>,
    /// Metric name with a measurement or series in the run.
    pub metric: Option<String>,
    /// Tag; repeat to require several.
    #[serde(default)]
    pub tag: Vec<String>,
    /// Valid runs by default (FR-9).
    #[serde(default)]
    pub validity: Validity,
    /// Page size, 1 to 500 (default 50).
    pub limit: Option<i64>,
    /// `next_cursor` of the previous page.
    pub cursor: Option<String>,
}

/// Longest `started_within_days` that is applied as given; longer spans
/// select every run anyway and would leave the range of timestamps.
const MAX_WITHIN_DAYS: i64 = 1_000_000;

impl RunFilter {
    /// Checks the relative selection and turns `started_within_days`,
    /// `since_tag` and `since_sut_tag` into the latest of them and
    /// `started_after`, which all must hold. The result no longer has those
    /// three fields; [`push_run_level_conditions`] expects a resolved filter.
    pub async fn resolve(&self, conn: &mut PgConnection) -> ApiResult<RunFilter> {
        if self.started_within_days.is_some_and(|n| n < 1) {
            return Err(ApiError::bad_request(
                "started_within_days must be at least 1",
            ));
        }
        if self.latest_runs.is_some_and(|n| n < 1) {
            return Err(ApiError::bad_request("latest_runs must be at least 1"));
        }
        let mut bounds: Vec<DateTime<Utc>> = self.started_after.into_iter().collect();
        if let Some(days) = self.started_within_days {
            bounds.push(Utc::now() - TimeDelta::days(days.min(MAX_WITHIN_DAYS)));
        }
        if let Some(tag) = &self.since_tag {
            let start: Option<DateTime<Utc>> = sqlx::query_scalar(
                "SELECT min(r.started_at) FROM run r \
                 JOIN run_tag t ON t.run_id = r.id WHERE t.tag = $1",
            )
            .bind(tag)
            .fetch_one(&mut *conn)
            .await?;
            bounds.push(start.ok_or_else(|| {
                ApiError::invalid(format!("since_tag: no run has the tag {tag:?}"))
            })?);
        }
        if let Some(tag) = &self.since_sut_tag {
            let start: Option<DateTime<Utc>> = sqlx::query_scalar(
                "SELECT min(r.started_at) FROM run r \
                 JOIN sut_version sut ON sut.id = r.sut_version_id WHERE sut.tag = $1",
            )
            .bind(tag)
            .fetch_one(&mut *conn)
            .await?;
            bounds.push(start.ok_or_else(|| {
                ApiError::invalid(format!(
                    "since_sut_tag: no run has a SUT version with the tag {tag:?}"
                ))
            })?);
        }
        Ok(RunFilter {
            started_after: bounds.into_iter().max(),
            started_within_days: None,
            since_tag: None,
            since_sut_tag: None,
            ..self.clone()
        })
    }

    fn is_resolved(&self) -> bool {
        self.started_within_days.is_none()
            && self.since_tag.is_none()
            && self.since_sut_tag.is_none()
    }
}

/// Parses `name=value` of a parameter filter.
pub fn parse_param_filter(filter: &str) -> ApiResult<(String, Value)> {
    let (name, raw) = filter.split_once('=').ok_or_else(|| {
        ApiError::bad_request(format!("param filter {filter:?} is not name=value"))
    })?;
    if name.trim().is_empty() {
        return Err(ApiError::bad_request(format!(
            "param filter {filter:?} has no name"
        )));
    }
    let value = match serde_json::from_str::<Value>(raw) {
        Ok(v @ (Value::Number(_) | Value::String(_) | Value::Bool(_))) => v,
        _ => Value::String(raw.to_owned()),
    };
    Ok((name.to_owned(), value))
}

fn encode_cursor(started_at: DateTime<Utc>, id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(format!("{}|{id}", started_at.to_rfc3339()))
}

fn decode_cursor(cursor: &str) -> ApiResult<(DateTime<Utc>, Uuid)> {
    let bad = || ApiError::bad_request("invalid cursor");
    let raw =
        String::from_utf8(URL_SAFE_NO_PAD.decode(cursor).map_err(|_| bad())?).map_err(|_| bad())?;
    let (ts, id) = raw.split_once('|').ok_or_else(bad)?;
    Ok((
        DateTime::parse_from_rfc3339(ts)
            .map_err(|_| bad())?
            .with_timezone(&Utc),
        id.parse().map_err(|_| bad())?,
    ))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SuiteRef {
    pub suite_version_id: i64,
    pub name: String,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SutVersion {
    pub id: i64,
    pub commit: String,
    pub branch: Option<String>,
    pub build_config: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunSummary {
    pub id: Uuid,
    pub suite: SuiteRef,
    pub sut_version: SutVersion,
    pub shared_environment_id: i64,
    pub triggered_by: RunTrigger,
    pub status: RunStatus,
    /// Measured start time.
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub is_valid: bool,
    pub invalid_reason: Option<String>,
    /// Upload time on the server.
    pub created_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct RunRow {
    id: Uuid,
    suite_version_id: i64,
    suite_name: String,
    suite_version: i32,
    sut_id: i64,
    sut_commit: String,
    sut_branch: Option<String>,
    sut_build_config: Option<String>,
    sut_tag: Option<String>,
    shared_environment_id: i64,
    triggered_by: RunTrigger,
    status: RunStatus,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    is_valid: bool,
    invalid_reason: Option<String>,
    created_at: DateTime<Utc>,
    tags: Vec<String>,
}

impl From<RunRow> for RunSummary {
    fn from(r: RunRow) -> Self {
        Self {
            id: r.id,
            suite: SuiteRef {
                suite_version_id: r.suite_version_id,
                name: r.suite_name,
                version: r.suite_version,
            },
            sut_version: SutVersion {
                id: r.sut_id,
                commit: r.sut_commit,
                branch: r.sut_branch,
                build_config: r.sut_build_config,
                tag: r.sut_tag,
            },
            shared_environment_id: r.shared_environment_id,
            triggered_by: r.triggered_by,
            status: r.status,
            started_at: r.started_at,
            ended_at: r.ended_at,
            is_valid: r.is_valid,
            invalid_reason: r.invalid_reason,
            created_at: r.created_at,
            tags: r.tags,
        }
    }
}

const RUN_SELECT: &str = "\
    SELECT r.id, r.suite_version_id, s.name AS suite_name, sv.version AS suite_version, \
           sut.id AS sut_id, sut.commit AS sut_commit, sut.branch AS sut_branch, \
           sut.build_config AS sut_build_config, sut.tag AS sut_tag, \
           r.shared_environment_id, r.triggered_by, r.status, r.started_at, r.ended_at, \
           r.is_valid, r.invalid_reason, r.created_at, \
           ARRAY(SELECT t.tag FROM run_tag t WHERE t.run_id = r.id ORDER BY t.tag) AS tags \
    FROM run r \
    JOIN suite_version sv ON sv.id = r.suite_version_id \
    JOIN suite s ON s.id = sv.suite_id \
    JOIN sut_version sut ON sut.id = r.sut_version_id";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunList {
    pub runs: Vec<RunSummary>,
    /// Pass as `cursor` to get the next page; absent on the last page.
    pub next_cursor: Option<String>,
}

/// Lists runs, newest measured start first.
pub async fn list_runs(pool: &PgPool, filter: &RunFilter) -> ApiResult<RunList> {
    let limit = filter.limit.unwrap_or(50);
    if !(1..=500).contains(&limit) {
        return Err(ApiError::bad_request("limit must be between 1 and 500"));
    }
    let filter = &filter.resolve(&mut *pool.acquire().await?).await?;
    let aliases = Aliases::load(pool).await?;
    let mut q = QueryBuilder::<Postgres>::new(RUN_SELECT);
    q.push(" WHERE true");
    push_run_conditions(&mut q, filter, &aliases)?;
    if let Some(cursor) = &filter.cursor {
        let (ts, id) = decode_cursor(cursor)?;
        q.push(" AND (r.started_at, r.id) < (")
            .push_bind(ts)
            .push(", ")
            .push_bind(id)
            .push(")");
    }
    q.push(" ORDER BY r.started_at DESC, r.id DESC LIMIT ")
        .push_bind(limit + 1);

    let mut rows: Vec<RunRow> = q.build_query_as().fetch_all(pool).await?;
    let next_cursor = if rows.len() as i64 > limit {
        rows.truncate(limit as usize);
        rows.last().map(|r| encode_cursor(r.started_at, r.id))
    } else {
        None
    };
    Ok(RunList {
        runs: rows.into_iter().map(Into::into).collect(),
        next_cursor,
    })
}

/// Appends the conditions of a resolved run filter ([`RunFilter::resolve`]),
/// `latest_runs` included. Expects `run r`, `suite s` and `sut_version sut`
/// in scope.
pub fn push_run_conditions(
    q: &mut QueryBuilder<'_, Postgres>,
    f: &RunFilter,
    aliases: &Aliases,
) -> ApiResult<()> {
    push_matching_run_conditions(q, f, aliases)?;
    push_latest_runs_condition(q, f, aliases)
}

/// Appends `latest_runs`: of the runs that match all other conditions of the
/// filter, only the N with the newest measured start (ties by id, as in the
/// run list). Expects `run r` in scope.
pub fn push_latest_runs_condition(
    q: &mut QueryBuilder<'_, Postgres>,
    f: &RunFilter,
    aliases: &Aliases,
) -> ApiResult<()> {
    let Some(n) = f.latest_runs else {
        return Ok(());
    };
    q.push(
        " AND r.id IN (SELECT r.id FROM run r \
          JOIN suite_version sv ON sv.id = r.suite_version_id \
          JOIN suite s ON s.id = sv.suite_id \
          JOIN sut_version sut ON sut.id = r.sut_version_id \
          WHERE true",
    );
    push_matching_run_conditions(q, f, aliases)?;
    q.push(" ORDER BY r.started_at DESC, r.id DESC LIMIT ")
        .push_bind(n)
        .push(")");
    Ok(())
}

/// The conditions of a filter except `latest_runs`: run-level ones, and a
/// current case execution that matches the others.
fn push_matching_run_conditions(
    q: &mut QueryBuilder<'_, Postgres>,
    f: &RunFilter,
    aliases: &Aliases,
) -> ApiResult<()> {
    push_run_level_conditions(q, f);
    let has_ce_filter = f.client_device.is_some()
        || f.scenario.is_some()
        || f.asset.is_some()
        || f.metric.is_some()
        || !f.param.is_empty();
    if !has_ce_filter {
        return Ok(());
    }
    q.push(
        " AND EXISTS (SELECT 1 FROM current_case_execution ce \
          JOIN test_case tc ON tc.id = ce.test_case_id \
          JOIN scenario_version scv ON scv.id = tc.scenario_version_id \
          JOIN scenario sc ON sc.id = scv.scenario_id \
          JOIN client_device cd ON cd.id = ce.client_device_id \
          WHERE ce.run_id = r.id",
    );
    push_case_execution_conditions(q, f, aliases)?;
    q.push(")");
    Ok(())
}

/// Appends the conditions of a resolved filter that apply to the run itself,
/// without `latest_runs`.
pub fn push_run_level_conditions(q: &mut QueryBuilder<'_, Postgres>, f: &RunFilter) {
    debug_assert!(f.is_resolved(), "run filter not resolved");
    match f.validity {
        Validity::Valid => q.push(" AND r.is_valid"),
        Validity::Invalid => q.push(" AND NOT r.is_valid"),
        Validity::All => q,
    };
    let text_conditions = [
        ("s.name", &f.suite),
        ("sut.commit", &f.sut_commit),
        ("sut.branch", &f.sut_branch),
        ("sut.tag", &f.sut_tag),
        ("sut.build_config", &f.sut_build_config),
    ];
    for (column, value) in text_conditions {
        if let Some(v) = value {
            q.push(format_args!(" AND {column} = "))
                .push_bind(v.clone());
        }
    }
    if let Some(t) = f.started_after {
        q.push(" AND r.started_at >= ").push_bind(t);
    }
    if let Some(t) = f.started_before {
        q.push(" AND r.started_at < ").push_bind(t);
    }
    if !f.status.is_empty() {
        q.push(" AND r.status = ANY(")
            .push_bind(f.status.clone())
            .push(")");
    }
    if let Some(t) = f.triggered_by {
        q.push(" AND r.triggered_by = ").push_bind(t);
    }
    if let Some(id) = f.shared_environment_id {
        q.push(" AND r.shared_environment_id = ").push_bind(id);
    }
    if !f.run_id.is_empty() {
        q.push(" AND r.id = ANY(")
            .push_bind(f.run_id.clone())
            .push(")");
    }
    for tag in &f.tag {
        q.push(" AND EXISTS (SELECT 1 FROM run_tag rt WHERE rt.run_id = r.id AND rt.tag = ")
            .push_bind(tag.clone())
            .push(")");
    }
}

/// Appends the case execution conditions of a filter (device, scenario,
/// asset, parameters, metric). Expects `current_case_execution ce`,
/// `test_case tc`, `scenario sc` and `client_device cd` in scope.
pub fn push_case_execution_conditions(
    q: &mut QueryBuilder<'_, Postgres>,
    f: &RunFilter,
    aliases: &Aliases,
) -> ApiResult<()> {
    if let Some(device) = &f.client_device {
        q.push(" AND cd.name = ANY(")
            .push_bind(aliases.raw_device_names(device))
            .push(")");
    }
    if let Some(scenario) = &f.scenario {
        q.push(" AND sc.name = ").push_bind(scenario.clone());
    }
    if let Some(asset) = &f.asset {
        q.push(
            " AND EXISTS (SELECT 1 FROM test_case_asset tca JOIN asset a ON a.id = tca.asset_id \
              WHERE tca.test_case_id = tc.id AND a.name = ",
        )
        .push_bind(asset.clone())
        .push(")");
    }
    for p in &f.param {
        let (name, value) = parse_param_filter(p)?;
        let raw = aliases.raw_values(&parameter_dimension(&name), &value);
        q.push(" AND tc.parameters -> ")
            .push_bind(name)
            .push(" IN (SELECT jsonb_array_elements(")
            .push_bind(Value::Array(raw))
            .push("))");
    }
    if let Some(metric) = &f.metric {
        q.push(
            " AND EXISTS (SELECT 1 FROM sample sa \
              JOIN metric mt ON mt.id IN ( \
                  SELECT m.metric_id FROM measurement m WHERE m.sample_id = sa.id \
                  UNION ALL SELECT se.metric_id FROM series se WHERE se.sample_id = sa.id) \
              WHERE sa.case_execution_id = ce.id AND mt.name = ",
        )
        .push_bind(metric.clone())
        .push(")");
    }
    Ok(())
}

// Run detail ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub run_id: Uuid,
    pub case_execution_id: Option<Uuid>,
    pub sample_id: Option<Uuid>,
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: i64,
    /// SHA-256 of the content, hex.
    pub sha256: String,
    pub created_at: DateTime<Utc>,
}

const ATTACHMENT_SELECT: &str = "\
    SELECT id, run_id, case_execution_id, sample_id, file_name, content_type, size_bytes, \
           encode(sha256, 'hex') AS sha256, created_at FROM attachment";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunDetail {
    #[serde(flatten)]
    pub run: RunSummary,
    /// Details of the shared environment.
    #[schema(value_type = Object)]
    pub shared_environment: Value,
    /// Last upload of anything for this run, in server time (FR-2).
    pub last_upload_at: DateTime<Utc>,
    pub case_execution_count: i64,
    pub annotations: Vec<Annotation>,
    pub attachments: Vec<AttachmentMeta>,
}

pub async fn get_run(pool: &PgPool, run_id: Uuid) -> ApiResult<RunDetail> {
    let row: RunRow = sqlx::query_as(&format!("{RUN_SELECT} WHERE r.id = $1"))
        .bind(run_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("run {run_id} not found")))?;
    let (shared_environment, last_upload_at, case_execution_count): (Value, DateTime<Utc>, i64) =
        sqlx::query_as(
            "SELECT e.details, run_last_upload_at(r.id), \
                    (SELECT count(*) FROM current_case_execution ce WHERE ce.run_id = r.id) \
             FROM run r JOIN environment e ON e.id = r.shared_environment_id WHERE r.id = $1",
        )
        .bind(run_id)
        .fetch_one(pool)
        .await?;
    let attachments = list_attachments(pool, run_id).await?;
    let annotations = annotations::for_run(pool, run_id).await?;
    Ok(RunDetail {
        run: row.into(),
        shared_environment,
        last_upload_at,
        case_execution_count,
        annotations,
        attachments,
    })
}

pub async fn list_attachments(pool: &PgPool, run_id: Uuid) -> ApiResult<Vec<AttachmentMeta>> {
    Ok(sqlx::query_as(&format!(
        "{ATTACHMENT_SELECT} WHERE run_id = $1 ORDER BY created_at, id"
    ))
    .bind(run_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_attachment(pool: &PgPool, id: Uuid) -> ApiResult<AttachmentMeta> {
    sqlx::query_as(&format!("{ATTACHMENT_SELECT} WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("attachment {id} not found")))
}

// Case executions -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AssetRef {
    pub id: i64,
    pub name: String,
    pub version: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestCaseSummary {
    pub id: i64,
    pub scenario: String,
    pub scenario_version: i32,
    pub scenario_version_id: i64,
    /// Parameters as uploaded.
    #[schema(value_type = Object)]
    pub parameters: Value,
    /// Parameters after aliases (FR-14).
    #[schema(value_type = Object)]
    pub resolved_parameters: Value,
    #[schema(value_type = Object)]
    pub parameter_units: Value,
    pub assets: Vec<AssetRef>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DeviceRef {
    pub id: i64,
    /// Stable name as uploaded.
    pub name: String,
    /// Name after aliases; trends and comparisons use this.
    pub resolved_name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseExecutionSummary {
    pub id: Uuid,
    pub run_id: Uuid,
    pub test_case: TestCaseSummary,
    pub client_device: DeviceRef,
    pub device_environment_id: i64,
    pub supersedes_id: Option<Uuid>,
    /// Set if a correction replaced this case execution.
    pub superseded_by: Option<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub sample_count: i64,
}

#[derive(sqlx::FromRow)]
struct CaseExecutionRow {
    id: Uuid,
    run_id: Uuid,
    test_case_id: i64,
    scenario: String,
    scenario_version: i32,
    scenario_version_id: i64,
    parameters: Value,
    parameter_units: Value,
    test_case_tags: Vec<String>,
    client_device_id: i64,
    device_name: String,
    device_environment_id: i64,
    supersedes_id: Option<Uuid>,
    superseded_by: Option<Uuid>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    sample_count: i64,
}

const CASE_EXECUTION_SELECT: &str = "\
    SELECT ce.id, ce.run_id, tc.id AS test_case_id, sc.name AS scenario, \
           scv.version AS scenario_version, scv.id AS scenario_version_id, \
           tc.parameters, tc.parameter_units, \
           ARRAY(SELECT t.tag FROM test_case_tag t WHERE t.test_case_id = tc.id ORDER BY t.tag) \
               AS test_case_tags, \
           cd.id AS client_device_id, cd.name AS device_name, ce.device_environment_id, \
           ce.supersedes_id, \
           (SELECT c2.id FROM case_execution c2 WHERE c2.supersedes_id = ce.id) AS superseded_by, \
           ce.started_at, ce.ended_at, ce.created_at, \
           (SELECT count(*) FROM sample sa WHERE sa.case_execution_id = ce.id) AS sample_count \
    FROM case_execution ce \
    JOIN test_case tc ON tc.id = ce.test_case_id \
    JOIN scenario_version scv ON scv.id = tc.scenario_version_id \
    JOIN scenario sc ON sc.id = scv.scenario_id \
    JOIN client_device cd ON cd.id = ce.client_device_id";

/// Assets of test cases, by test case id.
pub async fn test_case_assets(
    executor: impl sqlx::PgExecutor<'_>,
    test_case_ids: &[i64],
) -> ApiResult<HashMap<i64, Vec<AssetRef>>> {
    let rows: Vec<(i64, i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT tca.test_case_id, a.id, a.name, a.version, a.content_hash \
         FROM test_case_asset tca JOIN asset a ON a.id = tca.asset_id \
         WHERE tca.test_case_id = ANY($1) ORDER BY a.name, a.id",
    )
    .bind(test_case_ids)
    .fetch_all(executor)
    .await?;
    let mut out: HashMap<i64, Vec<AssetRef>> = HashMap::new();
    for (tc, id, name, version, content_hash) in rows {
        out.entry(tc).or_default().push(AssetRef {
            id,
            name,
            version,
            content_hash,
        });
    }
    Ok(out)
}

async fn case_execution_summaries(
    pool: &PgPool,
    rows: Vec<CaseExecutionRow>,
) -> ApiResult<Vec<CaseExecutionSummary>> {
    let aliases = Aliases::load(pool).await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.test_case_id).collect();
    let assets = test_case_assets(pool, &ids).await?;
    Ok(rows
        .into_iter()
        .map(|r| CaseExecutionSummary {
            id: r.id,
            run_id: r.run_id,
            test_case: TestCaseSummary {
                id: r.test_case_id,
                scenario: r.scenario,
                scenario_version: r.scenario_version,
                scenario_version_id: r.scenario_version_id,
                resolved_parameters: aliases.resolve_parameters(&r.parameters),
                parameters: r.parameters,
                parameter_units: r.parameter_units,
                assets: assets.get(&r.test_case_id).cloned().unwrap_or_default(),
                tags: r.test_case_tags,
            },
            client_device: DeviceRef {
                id: r.client_device_id,
                resolved_name: aliases.resolve_device(&r.device_name),
                name: r.device_name,
            },
            device_environment_id: r.device_environment_id,
            supersedes_id: r.supersedes_id,
            superseded_by: r.superseded_by,
            started_at: r.started_at,
            ended_at: r.ended_at,
            created_at: r.created_at,
            sample_count: r.sample_count,
        })
        .collect())
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams, schemars::JsonSchema)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct CaseExecutionListQuery {
    /// Include case executions replaced by a correction.
    #[serde(default)]
    pub include_superseded: bool,
}

pub async fn list_case_executions(
    pool: &PgPool,
    run_id: Uuid,
    query: &CaseExecutionListQuery,
) -> ApiResult<Vec<CaseExecutionSummary>> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM run WHERE id = $1)")
        .bind(run_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::not_found(format!("run {run_id} not found")));
    }
    let superseded = if query.include_superseded {
        ""
    } else {
        " AND NOT EXISTS (SELECT 1 FROM case_execution c3 WHERE c3.supersedes_id = ce.id)"
    };
    let rows: Vec<CaseExecutionRow> = sqlx::query_as(&format!(
        "{CASE_EXECUTION_SELECT} WHERE ce.run_id = $1{superseded} \
         ORDER BY sc.name, tc.id, cd.name, ce.created_at, ce.id"
    ))
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    case_execution_summaries(pool, rows).await
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MeasurementOut {
    pub metric: String,
    pub metric_id: i64,
    /// The unit the metric has now; the value was not converted.
    pub unit: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeriesMeta {
    pub id: Uuid,
    pub sample_id: Uuid,
    pub metric: String,
    pub metric_id: i64,
    /// Unit of y (the unit the metric has now).
    pub unit: String,
    pub x_dimension: String,
    pub x_unit: String,
    pub point_count: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SampleDetail {
    pub id: Uuid,
    pub repetition: i32,
    pub is_warmup: bool,
    pub created_at: DateTime<Utc>,
    pub measurements: Vec<MeasurementOut>,
    pub series: Vec<SeriesMeta>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseExecutionDetail {
    #[serde(flatten)]
    pub case_execution: CaseExecutionSummary,
    /// Details of the client device for this execution.
    #[schema(value_type = Object)]
    pub device_details: Value,
    pub samples: Vec<SampleDetail>,
}

pub async fn get_case_execution(pool: &PgPool, ce_id: Uuid) -> ApiResult<CaseExecutionDetail> {
    let row: CaseExecutionRow =
        sqlx::query_as(&format!("{CASE_EXECUTION_SELECT} WHERE ce.id = $1"))
            .bind(ce_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::not_found(format!("case execution {ce_id} not found")))?;
    let device_details: Value = sqlx::query_scalar("SELECT details FROM environment WHERE id = $1")
        .bind(row.device_environment_id)
        .fetch_one(pool)
        .await?;
    let summary = case_execution_summaries(pool, vec![row]).await?.remove(0);

    let samples: Vec<(Uuid, i32, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, repetition, is_warmup, created_at FROM sample \
         WHERE case_execution_id = $1 ORDER BY repetition",
    )
    .bind(ce_id)
    .fetch_all(pool)
    .await?;
    let measurements: Vec<(Uuid, String, i64, String, f64)> = sqlx::query_as(
        "SELECT m.sample_id, mt.name, mt.id, mt.unit, m.value \
         FROM measurement m JOIN sample sa ON sa.id = m.sample_id \
         JOIN metric mt ON mt.id = m.metric_id \
         WHERE sa.case_execution_id = $1 ORDER BY mt.name",
    )
    .bind(ce_id)
    .fetch_all(pool)
    .await?;
    let series = series_meta_where(pool, "sa.case_execution_id = $1", ce_id).await?;

    let mut by_sample: HashMap<Uuid, SampleDetail> = samples
        .iter()
        .map(|(id, repetition, is_warmup, created_at)| {
            (
                *id,
                SampleDetail {
                    id: *id,
                    repetition: *repetition,
                    is_warmup: *is_warmup,
                    created_at: *created_at,
                    measurements: vec![],
                    series: vec![],
                },
            )
        })
        .collect();
    for (sample_id, metric, metric_id, unit, value) in measurements {
        if let Some(s) = by_sample.get_mut(&sample_id) {
            s.measurements.push(MeasurementOut {
                metric,
                metric_id,
                unit,
                value,
            });
        }
    }
    for meta in series {
        if let Some(s) = by_sample.get_mut(&meta.sample_id) {
            s.series.push(meta);
        }
    }
    let samples = samples
        .iter()
        .filter_map(|(id, ..)| by_sample.remove(id))
        .collect();
    Ok(CaseExecutionDetail {
        case_execution: summary,
        device_details,
        samples,
    })
}

async fn series_meta_where(pool: &PgPool, condition: &str, id: Uuid) -> ApiResult<Vec<SeriesMeta>> {
    let rows: Vec<(Uuid, Uuid, String, i64, String, String, String, i32)> =
        sqlx::query_as(&format!(
            "SELECT se.id, se.sample_id, mt.name, mt.id, mt.unit, se.x_dimension, \
                    se.x_unit, se.point_count \
             FROM series se JOIN sample sa ON sa.id = se.sample_id \
             JOIN metric mt ON mt.id = se.metric_id \
             WHERE {condition} ORDER BY mt.name, se.id"
        ))
        .bind(id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, sample_id, metric, metric_id, unit, x_dimension, x_unit, point_count)| {
                SeriesMeta {
                    id,
                    sample_id,
                    metric,
                    metric_id,
                    unit,
                    x_dimension,
                    x_unit,
                    point_count,
                }
            },
        )
        .collect())
}

pub async fn get_series(pool: &PgPool, series_id: Uuid) -> ApiResult<SeriesMeta> {
    series_meta_where(pool, "se.id = $1", series_id)
        .await?
        .pop()
        .ok_or_else(|| ApiError::not_found(format!("series {series_id} not found")))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeriesPoints {
    pub id: Uuid,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct SeriesPointsQuery {
    /// Downsample for display to at most this many points, keeping the
    /// minimum and maximum of each bucket (FR-19). At least 2.
    pub max_points: Option<usize>,
}

pub async fn get_series_points(
    pool: &PgPool,
    series_id: Uuid,
    query: &SeriesPointsQuery,
) -> ApiResult<SeriesPoints> {
    let points = series_store::load_by_id(pool, series_id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("series {series_id} not found")))?;
    let Some(max_points) = query.max_points else {
        return Ok(SeriesPoints {
            id: series_id,
            x: points.x,
            y: points.y,
        });
    };
    if max_points < 2 {
        return Err(ApiError::bad_request("max_points must be at least 2"));
    }
    let reduced = Series::from_f32(&points.x, &points.y).downsample(max_points);
    Ok(SeriesPoints {
        id: series_id,
        x: reduced.x.iter().map(|v| *v as f32).collect(),
        y: reduced.y.iter().map(|v| *v as f32).collect(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use track_record_core::chart::VarType;
    use track_record_core::chart::filter::{FIELDS, STATUSES, TRIGGERS, VALIDITIES};

    use super::*;

    #[test]
    fn the_run_filter_of_chart_sources_has_only_fields_of_run_filter() {
        // One value per field of the table, of its type; enums take one of
        // their values.
        let mut fields = serde_json::Map::new();
        for (name, ty, list) in FIELDS {
            let one = match (*name, ty) {
                ("run_id", _) => json!("7c6c5a3e-0c4b-4d4e-9c39-6f1e6c3b8a10"),
                ("status", _) => json!("completed"),
                ("triggered_by", _) => json!("ci"),
                ("validity", _) => json!("all"),
                ("param", _) => json!("size=10"),
                (_, VarType::Number) => json!(5),
                (_, VarType::Time) => json!("2026-11-27T00:00:00Z"),
                (_, VarType::Boolean) => json!(true),
                _ => json!("main"),
            };
            let value = if *list { json!([one]) } else { one };
            fields.insert((*name).to_owned(), value);
        }
        let filter: RunFilter = serde_json::from_value(fields.into()).unwrap();
        assert_eq!(filter.sut_branch.as_deref(), Some("main"));
        assert_eq!(filter.latest_runs, Some(5));
        assert_eq!(filter.validity, Validity::All);
        assert_eq!(filter.limit, None);

        // Every value of an enumeration that charts accept is one here.
        for (field, values) in [
            ("status", STATUSES),
            ("triggered_by", TRIGGERS),
            ("validity", VALIDITIES),
        ] {
            for value in values {
                let v = if field == "status" {
                    json!([value])
                } else {
                    json!(value)
                };
                serde_json::from_value::<RunFilter>(json!({ field: v }))
                    .unwrap_or_else(|e| panic!("{field} {value}: {e}"));
            }
        }
    }

    #[test]
    fn param_filters_keep_json_types() {
        assert_eq!(
            parse_param_filter("size=10").unwrap(),
            ("size".into(), json!(10))
        );
        assert_eq!(
            parse_param_filter("mode=\"10\"").unwrap(),
            ("mode".into(), json!("10"))
        );
        assert_eq!(
            parse_param_filter("mode=fast").unwrap(),
            ("mode".into(), json!("fast"))
        );
        assert_eq!(
            parse_param_filter("on=true").unwrap(),
            ("on".into(), json!(true))
        );
        assert_eq!(
            parse_param_filter("a=[1]").unwrap(),
            ("a".into(), json!("[1]"))
        );
        assert_eq!(
            parse_param_filter("a=b=c").unwrap(),
            ("a".into(), json!("b=c"))
        );
        assert!(parse_param_filter("novalue").is_err());
        assert!(parse_param_filter("=1").is_err());
    }

    #[test]
    fn cursor_round_trip() {
        let t = DateTime::parse_from_rfc3339("2026-11-21T10:00:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        let id = Uuid::new_v4();
        assert_eq!(decode_cursor(&encode_cursor(t, id)).unwrap(), (t, id));
        assert!(decode_cursor("garbage!").is_err());
    }
}
