//! Derived metrics (FR-17, FR-17a): validation, saving, listing and
//! evaluation of DSL expressions (ADR 0009).
//!
//! References resolve as follows:
//! - a bare name is an uploaded scalar metric of the sample,
//! - `series.<name>` an uploaded series of the sample,
//! - `derived.<name>` a derived metric: the latest version when validating
//!   or evaluating an ad-hoc expression, the pinned version when evaluating
//!   a saved one.
//!
//! Validation checks units against the latest metric versions and the most
//! recent series of each referenced metric. Evaluation checks again with
//! the units of each sample, so a metric whose unit changed between
//! versions still converts correctly. Every result is converted to one
//! output unit: the declared unit of a saved metric, the requested unit, or
//! the unit the expression has with the latest units.

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgExecutor, PgPool};
use track_record_core::dsl::check::SeriesUnits;
use track_record_core::dsl::eval::{EvalError, Value};
use track_record_core::dsl::program::{
    Definition, Evaluator, Library, Root, SampleData, UnitSource, compile,
};
use track_record_core::dsl::{Diagnostic, Kind, References, Signature, parse};
use track_record_core::series::Series;
use track_record_core::units::UnitCatalogue;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::series_store;
use crate::service::catalog;

// Loading ------------------------------------------------------------------------

/// Latest version id of each named derived metric that exists.
async fn latest_versions(
    executor: impl PgExecutor<'_>,
    names: &BTreeSet<String>,
) -> ApiResult<HashMap<String, i64>> {
    let names: Vec<&String> = names.iter().collect();
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT name, derived_metric_version_id FROM latest_derived_metric_version \
         WHERE name = ANY($1)",
    )
    .bind(names)
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().collect())
}

/// The given derived metric versions and every version they use.
pub async fn load_library(pool: &PgPool, version_ids: &[i64]) -> ApiResult<Library> {
    if version_ids.is_empty() {
        return Ok(Library::default());
    }
    let rows: Vec<(i64, String, i32, String, String)> = sqlx::query_as(
        "WITH RECURSIVE closure(id) AS ( \
             SELECT unnest($1::bigint[]) \
             UNION \
             SELECT d.uses_version_id FROM derived_metric_dependency d \
             JOIN closure c ON c.id = d.derived_metric_version_id) \
         SELECT v.id, dm.name, v.version, v.expression, v.unit \
         FROM derived_metric_version v JOIN derived_metric dm ON dm.id = v.derived_metric_id \
         WHERE v.id IN (SELECT id FROM closure)",
    )
    .bind(version_ids)
    .fetch_all(pool)
    .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let deps: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT d.derived_metric_version_id, dm.name, d.uses_version_id \
         FROM derived_metric_dependency d JOIN derived_metric dm ON dm.id = d.uses_derived_metric_id \
         WHERE d.derived_metric_version_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let mut uses: HashMap<i64, HashMap<String, i64>> = HashMap::new();
    for (version, name, used) in deps {
        uses.entry(version).or_default().insert(name, used);
    }
    Ok(Library::new(rows.into_iter().map(
        |(version_id, name, version, expression, unit)| Definition {
            version_id,
            name,
            version,
            expression,
            unit,
            uses: uses.remove(&version_id).unwrap_or_default(),
        },
    )))
}

/// Units of the latest metric versions and of the most recent series of
/// each metric: what validation checks against.
#[derive(Debug, Clone, Default)]
pub struct LatestUnits {
    metrics: HashMap<String, String>,
    series: HashMap<String, SeriesUnits>,
}

impl LatestUnits {
    pub async fn load(pool: &PgPool, names: &BTreeSet<String>) -> ApiResult<Self> {
        let names: Vec<&String> = names.iter().collect();
        let metrics: Vec<(String, String)> = sqlx::query_as(
            "SELECT DISTINCT ON (m.id) m.name, v.unit FROM metric m \
             JOIN metric_version v ON v.metric_id = m.id \
             WHERE m.name = ANY($1) ORDER BY m.id, v.version DESC",
        )
        .bind(&names)
        .fetch_all(pool)
        .await?;
        let series: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT DISTINCT ON (m.id) m.name, v.unit, se.x_unit, se.x_dimension FROM metric m \
             JOIN metric_version v ON v.metric_id = m.id \
             JOIN series se ON se.metric_version_id = v.id \
             WHERE m.name = ANY($1) ORDER BY m.id, se.created_at DESC, se.key DESC",
        )
        .bind(&names)
        .fetch_all(pool)
        .await?;
        Ok(Self {
            metrics: metrics.into_iter().collect(),
            series: series
                .into_iter()
                .map(|(name, unit, x_unit, x_dimension)| {
                    (
                        name,
                        SeriesUnits {
                            unit,
                            x_unit,
                            x_dimension,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl UnitSource for LatestUnits {
    fn metric_unit(&self, name: &str) -> Option<String> {
        self.metrics.get(name).cloned()
    }

    fn series_units(&self, name: &str) -> Option<SeriesUnits> {
        self.series.get(name).cloned()
    }
}

/// Raw inputs of the given samples: the referenced measurements and series,
/// each in the unit it was uploaded in. Every sample gets an entry.
pub async fn load_sample_data(
    pool: &PgPool,
    sample_ids: &[Uuid],
    refs: &References,
) -> ApiResult<HashMap<Uuid, SampleData>> {
    let mut out: HashMap<Uuid, SampleData> = sample_ids
        .iter()
        .map(|id| (*id, SampleData::default()))
        .collect();
    if !refs.metrics.is_empty() {
        let names: Vec<&String> = refs.metrics.iter().collect();
        let rows: Vec<(Uuid, String, String, f64)> = sqlx::query_as(
            "SELECT m.sample_id, mt.name, mv.unit, m.value FROM measurement m \
             JOIN metric_version mv ON mv.id = m.metric_version_id \
             JOIN metric mt ON mt.id = mv.metric_id \
             WHERE m.sample_id = ANY($1) AND mt.name = ANY($2)",
        )
        .bind(sample_ids)
        .bind(names)
        .fetch_all(pool)
        .await?;
        for (sample, name, unit, value) in rows {
            if let Some(d) = out.get_mut(&sample) {
                d.measurements.insert(name, (value, unit));
            }
        }
    }
    if !refs.series.is_empty() {
        let names: Vec<&String> = refs.series.iter().collect();
        let rows: Vec<(Uuid, Uuid, String, String, String, String)> = sqlx::query_as(
            "SELECT se.id, se.sample_id, mt.name, mv.unit, se.x_unit, se.x_dimension \
             FROM series se JOIN metric_version mv ON mv.id = se.metric_version_id \
             JOIN metric mt ON mt.id = mv.metric_id \
             WHERE se.sample_id = ANY($1) AND mt.name = ANY($2)",
        )
        .bind(sample_ids)
        .bind(names)
        .fetch_all(pool)
        .await?;
        let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
        let mut points = series_store::load_many(pool, &ids).await?;
        for (id, sample, name, unit, x_unit, x_dimension) in rows {
            let p = points.remove(&id).unwrap_or_default();
            if let Some(d) = out.get_mut(&sample) {
                d.series.insert(
                    name,
                    (
                        Series::from_f32(&p.x, &p.y),
                        SeriesUnits {
                            unit,
                            x_unit,
                            x_dimension,
                        },
                    ),
                );
            }
        }
    }
    Ok(out)
}

// Validation -------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DslDiagnostic {
    pub message: String,
    /// Byte offsets of the part of the expression the message refers to.
    pub start: usize,
    pub end: usize,
}

impl From<&Diagnostic> for DslDiagnostic {
    fn from(d: &Diagnostic) -> Self {
        Self {
            message: d.message.clone(),
            start: d.span.start,
            end: d.span.end,
        }
    }
}

/// A derived metric version an expression uses.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UsedVersion {
    pub name: String,
    pub version: i32,
    pub version_id: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExpressionInputs {
    /// Uploaded scalar metrics the expression needs (including through the
    /// derived metrics it uses).
    pub metrics: Vec<String>,
    /// Uploaded series it needs.
    pub series: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Validation {
    pub valid: bool,
    pub diagnostics: Vec<DslDiagnostic>,
    /// `scalar` or `series`, if valid.
    pub kind: Option<String>,
    /// Unit of the result (the declared unit if one was given), if valid.
    pub unit: Option<String>,
    /// Unit the expression has by itself, if valid.
    pub inferred_unit: Option<String>,
    /// For a series: unit and dimension of x.
    pub x_unit: Option<String>,
    pub x_dimension: Option<String>,
    /// Derived metric versions referenced directly (latest versions).
    pub uses: Vec<UsedVersion>,
    pub inputs: ExpressionInputs,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateExpression {
    pub expression: String,
    /// Unit the result must convert to.
    pub unit: Option<String>,
    /// Name the expression will be saved under; a reference to it, direct
    /// or through other derived metrics, is a cycle.
    pub name: Option<String>,
}

/// An expression analysed against the current definitions.
pub struct Analysis {
    pub library: Library,
    pub catalogue: UnitCatalogue,
    pub root: Option<Root>,
    pub result: Result<Signature, Vec<Diagnostic>>,
    pub inferred_unit: String,
    pub uses: Vec<UsedVersion>,
    pub inputs: References,
}

/// Parses and checks an expression with the latest definitions and units.
pub async fn analyze(
    pool: &PgPool,
    expression: &str,
    unit: Option<&str>,
    name: Option<&str>,
) -> ApiResult<Analysis> {
    let catalogue = catalog::unit_catalogue(pool).await?;
    let expr = match parse(expression) {
        Ok(e) => e,
        Err(d) => {
            return Ok(Analysis {
                library: Library::default(),
                catalogue,
                root: None,
                result: Err(vec![d]),
                inferred_unit: String::new(),
                uses: vec![],
                inputs: References::default(),
            });
        }
    };
    let refs = expr.references();
    let uses = latest_versions(pool, &refs.derived).await?;
    let ids: Vec<i64> = uses.values().copied().collect();
    let library = load_library(pool, &ids).await?;

    let mut inputs = refs.clone();
    inputs.derived.clear();
    for id in library_ids(&library, &ids) {
        if let Some(d) = library.get(id)
            && let Ok(e) = parse(&d.expression)
        {
            let r = e.references();
            inputs.metrics.extend(r.metrics);
            inputs.series.extend(r.series);
        }
    }
    let names: BTreeSet<String> = inputs.metrics.union(&inputs.series).cloned().collect();
    let latest = LatestUnits::load(pool, &names).await?;

    let root = Root {
        expr,
        uses: uses.clone(),
        unit: unit.map(str::to_owned),
    };
    let mut result = compile(&library, &catalogue, &root, &latest);
    let mut inferred_unit = String::new();
    if let Ok(c) = &result {
        inferred_unit = c.root.inferred_unit.clone();
    }
    if let Some(name) = name
        && let Some(diag) = cycle(&library, &root, name)
    {
        match &mut result {
            Ok(_) => result = Err(vec![diag]),
            Err(d) => d.push(diag),
        }
    }
    let mut used: Vec<UsedVersion> = uses
        .iter()
        .filter_map(|(name, id)| {
            library.get(*id).map(|d| UsedVersion {
                name: name.clone(),
                version: d.version,
                version_id: *id,
            })
        })
        .collect();
    used.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Analysis {
        library,
        catalogue,
        result: result.map(|c| c.root.signature),
        root: Some(root),
        inferred_unit,
        uses: used,
        inputs,
    })
}

/// All version ids in the library reachable from `ids` (the library holds
/// exactly that closure).
fn library_ids(library: &Library, ids: &[i64]) -> BTreeSet<i64> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<i64> = ids.to_vec();
    while let Some(id) = stack.pop() {
        if seen.insert(id)
            && let Some(d) = library.get(id)
        {
            stack.extend(d.uses.values().copied());
        }
    }
    seen
}

/// A diagnostic if the expression would use the derived metric `name`.
fn cycle(library: &Library, root: &Root, name: &str) -> Option<Diagnostic> {
    for (used, id) in &root.uses {
        let closure = library_ids(library, &[*id]);
        if used == name
            || closure
                .iter()
                .any(|v| library.get(*v).is_some_and(|d| d.name == name))
        {
            let span = find_reference(&root.expr, used).unwrap_or(root.expr.span);
            return Some(Diagnostic::new(
                span,
                format!("derived.{used} uses derived.{name}: a derived metric cannot use itself"),
            ));
        }
    }
    None
}

fn find_reference(
    expr: &track_record_core::dsl::Expr,
    name: &str,
) -> Option<track_record_core::dsl::Span> {
    use track_record_core::dsl::ExprKind;
    match &expr.kind {
        ExprKind::Derived(n) if n == name => Some(expr.span),
        ExprKind::Neg(e) => find_reference(e, name),
        ExprKind::Binary { lhs, rhs, .. } => {
            find_reference(lhs, name).or_else(|| find_reference(rhs, name))
        }
        ExprKind::Call { args, named, .. } => args
            .iter()
            .chain(named.iter().map(|n| &n.value))
            .find_map(|e| find_reference(e, name)),
        _ => None,
    }
}

pub async fn validate(pool: &PgPool, req: &ValidateExpression) -> ApiResult<Validation> {
    let a = analyze(
        pool,
        &req.expression,
        req.unit.as_deref(),
        req.name.as_deref(),
    )
    .await?;
    let (valid, diagnostics, sig) = match &a.result {
        Ok(sig) => (true, vec![], Some(sig)),
        Err(d) => (false, d.iter().map(Into::into).collect(), None),
    };
    let series = sig.is_some_and(|s| s.kind == Kind::Series);
    Ok(Validation {
        valid,
        diagnostics,
        kind: sig.map(|s| s.kind.as_str().to_owned()),
        unit: sig.map(|s| s.unit.clone()),
        inferred_unit: sig.map(|_| a.inferred_unit.clone()),
        x_unit: sig.filter(|_| series).map(|s| s.x_unit.clone()),
        x_dimension: sig.filter(|_| series).map(|s| s.x_dimension.clone()),
        uses: a.uses,
        inputs: ExpressionInputs {
            metrics: a.inputs.metrics.into_iter().collect(),
            series: a.inputs.series.into_iter().collect(),
        },
    })
}

fn invalid(expression: &str, diagnostics: &[Diagnostic]) -> ApiError {
    let parts: Vec<String> = diagnostics
        .iter()
        .map(|d| {
            let at = expression.get(d.span.start..d.span.end).unwrap_or("");
            if at.is_empty() {
                d.message.clone()
            } else {
                format!("{} (at {at:?})", d.message)
            }
        })
        .collect();
    ApiError::new(
        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_expression",
        format!("invalid expression: {}", parts.join("; ")),
    )
}

/// An expression ready to be evaluated on many samples.
pub struct Prepared {
    pub library: Library,
    pub catalogue: UnitCatalogue,
    pub root: Root,
    /// Kind, output unit and x with the latest units.
    pub signature: Signature,
    pub expression: String,
}

impl Prepared {
    pub fn evaluator(&self) -> Evaluator<'_> {
        Evaluator::new(&self.library, &self.catalogue, self.root.clone())
            .expect("the closure was resolved when preparing")
    }

    /// Factor converting an x unit of a sample to the x unit of the
    /// signature.
    pub fn x_factor(&self, x_unit: &str) -> Option<f64> {
        self.catalogue
            .conversion_factor(x_unit, &self.signature.x_unit)
    }
}

/// Prepares an ad-hoc expression (an uploaded metric, a derived metric or
/// any DSL expression) for analysis. `unit` converts the results; without
/// it they are in the unit the expression has with the latest units.
pub async fn prepare(pool: &PgPool, expression: &str, unit: Option<&str>) -> ApiResult<Prepared> {
    let a = analyze(pool, expression, unit, None).await?;
    let signature = a.result.map_err(|d| invalid(expression, &d))?;
    let mut root = a.root.expect("a valid expression has a root");
    root.unit = Some(signature.unit.clone());
    root.closure(&a.library).map_err(ApiError::invalid)?;
    Ok(Prepared {
        library: a.library,
        catalogue: a.catalogue,
        root,
        signature,
        expression: expression.to_owned(),
    })
}

// Saving and listing ----------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveDerivedMetric {
    pub expression: String,
    /// Unit of the result; the expression's unit must convert to it.
    pub unit: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DerivedMetricVersion {
    pub id: i64,
    pub name: String,
    pub version: i32,
    pub expression: String,
    pub unit: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    /// Exact versions of the derived metrics it uses (FR-17).
    pub uses: Vec<UsedVersion>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DerivedMetric {
    pub id: i64,
    pub name: String,
    /// Oldest first.
    pub versions: Vec<DerivedMetricVersion>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SavedDerivedMetric {
    /// False if the content equals the latest version, which is returned.
    pub created: bool,
    #[serde(flatten)]
    pub version: DerivedMetricVersion,
}

fn check_name(name: &str) -> ApiResult<()> {
    if name.trim().is_empty() || name != name.trim() || name.contains('`') {
        return Err(ApiError::invalid(
            "a derived metric name must not be empty, start or end with spaces, or contain '`'",
        ));
    }
    Ok(())
}

/// Saves a new version unless the content equals the latest one (FR-7).
/// The expression is validated first (FR-17a); the versions it uses are
/// pinned.
pub async fn save(
    pool: &PgPool,
    name: &str,
    req: &SaveDerivedMetric,
) -> ApiResult<SavedDerivedMetric> {
    check_name(name)?;
    if req.expression.trim().is_empty() {
        return Err(ApiError::invalid("expression must not be empty"));
    }
    let a = analyze(pool, &req.expression, Some(&req.unit), Some(name)).await?;
    if let Err(d) = &a.result {
        return Err(invalid(&req.expression, d));
    }
    let uses: Vec<i64> = a.uses.iter().map(|u| u.version_id).collect();
    let mut tx = pool.begin().await?;
    let before: Option<i64> = sqlx::query_scalar(
        "SELECT derived_metric_version_id FROM latest_derived_metric_version WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(&mut *tx)
    .await?;
    let id: i64 = sqlx::query_scalar("SELECT save_derived_metric($1, $2, $3, $4, $5)")
        .bind(name)
        .bind(&req.expression)
        .bind(&req.unit)
        .bind(&req.description)
        .bind(&uses)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    let version = versions_where(pool, "v.id = $1", id)
        .await?
        .pop()
        .ok_or_else(|| {
            ApiError::Internal(anyhow::anyhow!(
                "saved derived metric version {id} vanished"
            ))
        })?;
    Ok(SavedDerivedMetric {
        created: before != Some(id),
        version,
    })
}

async fn versions_where(
    pool: &PgPool,
    condition: &str,
    arg: i64,
) -> ApiResult<Vec<DerivedMetricVersion>> {
    let rows: Vec<(i64, String, i32, String, String, String, DateTime<Utc>)> =
        sqlx::query_as(&format!(
            "SELECT v.id, dm.name, v.version, v.expression, v.unit, v.description, v.created_at \
         FROM derived_metric_version v JOIN derived_metric dm ON dm.id = v.derived_metric_id \
         WHERE {condition} ORDER BY dm.name, v.version"
        ))
        .bind(arg)
        .fetch_all(pool)
        .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let deps: Vec<(i64, String, i32, i64)> = sqlx::query_as(
        "SELECT d.derived_metric_version_id, dm.name, v.version, v.id \
         FROM derived_metric_dependency d JOIN derived_metric_version v ON v.id = d.uses_version_id \
         JOIN derived_metric dm ON dm.id = v.derived_metric_id \
         WHERE d.derived_metric_version_id = ANY($1) ORDER BY dm.name",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let mut uses: HashMap<i64, Vec<UsedVersion>> = HashMap::new();
    for (of, name, version, version_id) in deps {
        uses.entry(of).or_default().push(UsedVersion {
            name,
            version,
            version_id,
        });
    }
    Ok(rows
        .into_iter()
        .map(
            |(id, name, version, expression, unit, description, created_at)| DerivedMetricVersion {
                uses: uses.remove(&id).unwrap_or_default(),
                id,
                name,
                version,
                expression,
                unit,
                description,
                created_at,
            },
        )
        .collect())
}

fn group(versions: Vec<DerivedMetricVersion>, ids: &HashMap<String, i64>) -> Vec<DerivedMetric> {
    let mut out: Vec<DerivedMetric> = Vec::new();
    for v in versions {
        if out.last().is_none_or(|m| m.name != v.name) {
            out.push(DerivedMetric {
                id: ids.get(&v.name).copied().unwrap_or_default(),
                name: v.name.clone(),
                versions: vec![],
            });
        }
        out.last_mut().expect("just pushed").versions.push(v);
    }
    out
}

async fn metric_ids(pool: &PgPool) -> ApiResult<HashMap<String, i64>> {
    let rows: Vec<(String, i64)> = sqlx::query_as("SELECT name, id FROM derived_metric")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

pub async fn list(pool: &PgPool) -> ApiResult<Vec<DerivedMetric>> {
    let versions = versions_where(pool, "$1 = $1", 0).await?;
    Ok(group(versions, &metric_ids(pool).await?))
}

pub async fn get(pool: &PgPool, name: &str) -> ApiResult<DerivedMetric> {
    let id: i64 = sqlx::query_scalar("SELECT id FROM derived_metric WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("derived metric {name} not found")))?;
    let versions = versions_where(pool, "dm.id = $1", id).await?;
    Ok(group(versions, &HashMap::from([(name.to_owned(), id)]))
        .pop()
        .expect("a derived metric has at least one version"))
}

// Evaluation on one case execution -------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluateRequest {
    pub case_execution_id: Uuid,
    /// An ad-hoc expression. Give either this or `name`.
    pub expression: Option<String>,
    /// A saved derived metric.
    pub name: Option<String>,
    /// Version of `name`; the latest by default.
    pub version: Option<i32>,
    /// Output unit; default: the declared unit, or the expression's unit.
    pub unit: Option<String>,
    /// For series results: downsample to at most this many points.
    pub max_points: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SampleStatus {
    /// The sample has a value.
    Value,
    /// The expression has no value for this sample (FR-12a).
    NoValue,
    /// The sample lacks an input.
    Missing,
    /// The inputs do not fit the expression (e.g. units or x values).
    Error,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeriesValue {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SampleResult {
    pub sample_id: Uuid,
    pub repetition: i32,
    pub is_warmup: bool,
    pub status: SampleStatus,
    /// Scalar result.
    pub value: Option<f64>,
    /// Series result, x in `x_unit`.
    pub series: Option<SeriesValue>,
    /// Why the status is `missing` or `error`.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EvaluateResponse {
    pub expression: String,
    pub kind: String,
    pub unit: String,
    pub x_unit: Option<String>,
    pub x_dimension: Option<String>,
    pub samples: Vec<SampleResult>,
}

/// Classifies the result of one sample.
pub fn classify(
    result: Result<track_record_core::dsl::program::Outcome, EvalError>,
) -> (
    SampleStatus,
    Option<Value>,
    Option<Signature>,
    Option<String>,
) {
    match result {
        Ok(out) => {
            let status = match &out.value {
                Value::Scalar(None) | Value::Series(None) => SampleStatus::NoValue,
                _ => SampleStatus::Value,
            };
            (status, Some(out.value), Some(out.signature), None)
        }
        Err(e @ EvalError::Missing(_)) => (SampleStatus::Missing, None, None, Some(e.to_string())),
        Err(e @ EvalError::Data(_)) => (SampleStatus::Error, None, None, Some(e.to_string())),
    }
}

/// Prepares a saved version for evaluation, in its declared unit or `unit`.
async fn prepare_saved(
    pool: &PgPool,
    name: &str,
    version: Option<i32>,
    unit: Option<&str>,
) -> ApiResult<Prepared> {
    let metric = get(pool, name).await?;
    let v = match version {
        Some(n) => metric
            .versions
            .iter()
            .find(|v| v.version == n)
            .ok_or_else(|| {
                ApiError::not_found(format!("derived metric {name} has no version {n}"))
            })?,
        None => metric.versions.last().expect("at least one version"),
    };
    let library = load_library(pool, &[v.id]).await?;
    let catalogue = catalog::unit_catalogue(pool).await?;
    let mut root = Root::definition(&library, v.id).map_err(ApiError::invalid)?;
    let (inputs, _) = root.closure(&library).map_err(ApiError::invalid)?;
    let names: BTreeSet<String> = inputs.metrics.union(&inputs.series).cloned().collect();
    let latest = LatestUnits::load(pool, &names).await?;
    if let Some(unit) = unit {
        root.unit = Some(unit.to_owned());
    }
    let compiled =
        compile(&library, &catalogue, &root, &latest).map_err(|d| invalid(&v.expression, &d))?;
    Ok(Prepared {
        signature: compiled.root.signature,
        library,
        catalogue,
        root,
        expression: format!("derived.{name}@{}", v.version),
    })
}

/// Evaluates an expression or a saved derived metric on every sample of a
/// case execution (warmup samples included).
pub async fn evaluate(pool: &PgPool, req: &EvaluateRequest) -> ApiResult<EvaluateResponse> {
    let prepared = match (&req.expression, &req.name) {
        (Some(e), None) if req.version.is_none() => prepare(pool, e, req.unit.as_deref()).await?,
        (None, Some(name)) => prepare_saved(pool, name, req.version, req.unit.as_deref()).await?,
        _ => {
            return Err(ApiError::bad_request(
                "give either expression or name (with an optional version)",
            ));
        }
    };
    if req.max_points.is_some_and(|m| m < 2) {
        return Err(ApiError::bad_request("max_points must be at least 2"));
    }
    let samples: Vec<(Uuid, i32, bool)> = sqlx::query_as(
        "SELECT id, repetition, is_warmup FROM sample WHERE case_execution_id = $1 ORDER BY repetition",
    )
    .bind(req.case_execution_id)
    .fetch_all(pool)
    .await?;
    if samples.is_empty() {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM case_execution WHERE id = $1)")
                .bind(req.case_execution_id)
                .fetch_one(pool)
                .await?;
        if !exists {
            return Err(ApiError::not_found(format!(
                "case execution {} not found",
                req.case_execution_id
            )));
        }
    }
    let mut evaluator = prepared.evaluator();
    let ids: Vec<Uuid> = samples.iter().map(|s| s.0).collect();
    let data = load_sample_data(pool, &ids, evaluator.inputs()).await?;
    let sig = &prepared.signature;
    let mut out = Vec::new();
    for (id, repetition, is_warmup) in samples {
        let (mut status, value, sample_sig, mut message) = classify(evaluator.evaluate(&data[&id]));
        let mut scalar = None;
        let mut series = None;
        match value {
            Some(Value::Scalar(v)) => scalar = v,
            Some(Value::Series(Some(s))) => {
                let xf = sample_sig
                    .as_ref()
                    .and_then(|s| prepared.x_factor(&s.x_unit));
                match xf {
                    Some(f) => {
                        let s = s.scale_x(f);
                        let s = match req.max_points {
                            Some(m) => s.downsample(m),
                            None => s,
                        };
                        series = Some(SeriesValue { x: s.x, y: s.y });
                    }
                    None => {
                        status = SampleStatus::Error;
                        message = Some(format!(
                            "x is in {:?}, which does not convert to {:?}",
                            sample_sig.map(|s| s.x_unit).unwrap_or_default(),
                            sig.x_unit
                        ));
                    }
                }
            }
            _ => {}
        }
        out.push(SampleResult {
            sample_id: id,
            repetition,
            is_warmup,
            status,
            value: scalar,
            series,
            message,
        });
    }
    let is_series = sig.kind == Kind::Series;
    Ok(EvaluateResponse {
        expression: prepared.expression.clone(),
        kind: sig.kind.as_str().to_owned(),
        unit: sig.unit.clone(),
        x_unit: is_series.then(|| sig.x_unit.clone()),
        x_dimension: is_series.then(|| sig.x_dimension.clone()),
        samples: out,
    })
}
