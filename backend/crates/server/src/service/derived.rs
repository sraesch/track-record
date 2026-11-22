//! Derived metrics (FR-17, FR-17a): validation, saving, listing and
//! evaluation of DSL expressions (ADR 0009).
//!
//! References resolve as follows:
//! - a bare name is an uploaded scalar metric of the sample,
//! - `series.<name>` an uploaded series of the sample,
//! - `derived.<name>` a derived metric. It has one definition, which can
//!   be changed (ADR 0017): a change applies to every later evaluation,
//!   historical runs included, and to the derived metrics built on it.
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
use sqlx::PgPool;
use track_record_core::dsl::check::SeriesUnits;
use track_record_core::dsl::eval::{EvalError, Value};
use track_record_core::dsl::program::{
    Definition, Evaluator, Library, Root, SampleData, UnitSource, compile,
};
use track_record_core::dsl::{
    Diagnostic, Expr, ExprKind, Kind, References, Signature, Span, parse, rename_derived,
};
use track_record_core::series::Series;
use track_record_core::units::UnitCatalogue;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::series_store;
use crate::service::catalog;

// Loading ------------------------------------------------------------------------

/// The named derived metrics that exist and every derived metric they use.
pub async fn load_library(pool: &PgPool, names: &BTreeSet<String>) -> ApiResult<Library> {
    if names.is_empty() {
        return Ok(Library::default());
    }
    let names: Vec<&String> = names.iter().collect();
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "WITH RECURSIVE closure(id) AS ( \
             SELECT id FROM derived_metric WHERE name = ANY($1) \
             UNION \
             SELECT d.uses_derived_metric_id FROM derived_metric_dependency d \
             JOIN closure c ON c.id = d.derived_metric_id) \
         SELECT name, expression, unit FROM derived_metric \
         WHERE id IN (SELECT id FROM closure)",
    )
    .bind(names)
    .fetch_all(pool)
    .await?;
    Ok(Library::new(rows.into_iter().map(
        |(name, expression, unit)| Definition {
            name,
            expression,
            unit,
        },
    )))
}

/// The derived metrics built on `name`, directly or through others.
async fn users_of(pool: &PgPool, name: &str) -> ApiResult<BTreeSet<String>> {
    let rows: Vec<String> = sqlx::query_scalar(
        "WITH RECURSIVE users(id) AS ( \
             SELECT d.derived_metric_id FROM derived_metric_dependency d \
             JOIN derived_metric m ON m.id = d.uses_derived_metric_id WHERE m.name = $1 \
             UNION \
             SELECT d.derived_metric_id FROM derived_metric_dependency d \
             JOIN users u ON u.id = d.uses_derived_metric_id) \
         SELECT name FROM derived_metric WHERE id IN (SELECT id FROM users)",
    )
    .bind(name)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
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
    /// Names of the derived metrics the expression refers to directly.
    pub uses: Vec<String>,
    pub inputs: ExpressionInputs,
}

#[derive(Debug, Clone, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateExpression {
    pub expression: String,
    /// Unit the result must convert to.
    pub unit: Option<String>,
    /// Name the expression will be saved under. A reference to it, direct
    /// or through other derived metrics, is a cycle. If the derived metric
    /// exists, the metrics built on it are checked too: the expression is
    /// invalid if it would break one of them.
    pub name: Option<String>,
}

/// An expression analysed against the current definitions.
pub struct Analysis {
    pub library: Library,
    pub catalogue: UnitCatalogue,
    pub root: Option<Root>,
    pub result: Result<Signature, Vec<Diagnostic>>,
    pub inferred_unit: String,
    /// The derived metrics referenced directly that exist, sorted.
    pub uses: Vec<String>,
    pub inputs: References,
}

/// Parses and checks an expression with the current definitions and the
/// latest units. With `name`, the expression is checked as the definition
/// of that derived metric: it must not use itself, and it must not break a
/// derived metric built on it.
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
    let library = load_library(pool, &refs.derived).await?;

    let root = Root {
        expr,
        unit: unit.map(str::to_owned),
    };
    let mut inputs = refs.clone();
    inputs.derived.clear();
    // The raw inputs of everything the expression uses, directly or not.
    let mut used = library.uses_closure(refs.derived.iter().cloned());
    used.extend(refs.derived.iter().cloned());
    for used in used {
        if let Some(d) = library.get(&used)
            && let Ok(e) = parse(&d.expression)
        {
            let r = e.references();
            inputs.metrics.extend(r.metrics);
            inputs.series.extend(r.series);
        }
    }
    let names: BTreeSet<String> = inputs.metrics.union(&inputs.series).cloned().collect();
    let latest = LatestUnits::load(pool, &names).await?;

    let mut result = match name.and_then(|name| cycle(&library, &root, name)) {
        // With a cycle there is nothing more to check: the metrics in it
        // would be checked with the stored definition of `name`, not with
        // this expression.
        Some(diagnostic) => Err(vec![diagnostic]),
        None => compile(&library, &catalogue, &root, &latest).map(|c| c.root),
    };
    let mut inferred_unit = String::new();
    if let Ok(checked) = &result {
        inferred_unit = checked.inferred_unit.clone();
    }
    if let (Some(name), Ok(checked)) = (name, &result) {
        let definition = Definition {
            name: name.to_owned(),
            expression: expression.to_owned(),
            unit: checked.signature.unit.clone(),
        };
        let broken = broken_users(pool, &catalogue, &library, definition, root.expr.span).await?;
        if !broken.is_empty() {
            result = Err(broken);
        }
    }
    let uses = refs
        .derived
        .iter()
        .filter(|used| library.get(used).is_some())
        .cloned()
        .collect();
    Ok(Analysis {
        library,
        catalogue,
        result: result.map(|c| c.signature),
        root: Some(root),
        inferred_unit,
        uses,
        inputs,
    })
}

/// A diagnostic if the expression would use the derived metric `name`,
/// which it is the definition of.
fn cycle(library: &Library, root: &Root, name: &str) -> Option<Diagnostic> {
    for used in root.expr.references().derived {
        if used == name || library.uses_closure([used.clone()]).contains(name) {
            let span = find_reference(&root.expr, &used).unwrap_or(root.expr.span);
            return Some(Diagnostic::new(
                span,
                format!("derived.{used} uses derived.{name}: a derived metric cannot use itself"),
            ));
        }
    }
    None
}

/// A derived metric has no versions, so a changed definition takes effect
/// in the derived metrics built on it. One diagnostic for each of them that
/// is valid now and would not be with `definition`, reported at `span`.
/// `library` holds what the new definition uses.
async fn broken_users(
    pool: &PgPool,
    catalogue: &UnitCatalogue,
    library: &Library,
    definition: Definition,
    span: Span,
) -> ApiResult<Vec<Diagnostic>> {
    let users = users_of(pool, &definition.name).await?;
    if users.is_empty() {
        return Ok(vec![]);
    }
    let name = definition.name.clone();
    let before = load_library(pool, &users).await?;
    let mut after = before.clone();
    after.merge(library);
    after.insert(definition);

    let check = |library: &Library, user: &str, units: &LatestUnits| -> Result<(), String> {
        let root = Root::definition(library, user)?;
        compile(library, catalogue, &root, units)
            .map(|_| ())
            .map_err(|d| d[0].message.clone())
    };
    let mut names = BTreeSet::new();
    for library in [&before, &after] {
        for user in &users {
            if let Ok(root) = Root::definition(library, user)
                && let Ok((raw, _)) = root.closure(library)
            {
                names.extend(raw.metrics);
                names.extend(raw.series);
            }
        }
    }
    let latest = LatestUnits::load(pool, &names).await?;
    let mut out = Vec::new();
    for user in &users {
        if check(&before, user, &latest).is_ok()
            && let Err(message) = check(&after, user, &latest)
        {
            out.push(Diagnostic::new(
                span,
                format!(
                    "derived.{user} uses derived.{name} and would no longer be valid: {message}"
                ),
            ));
        }
    }
    Ok(out)
}

fn find_reference(expr: &Expr, name: &str) -> Option<Span> {
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

/// A derived metric (FR-17). It has one definition, which can be changed.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DerivedMetric {
    pub id: i64,
    pub name: String,
    pub expression: String,
    pub unit: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    /// When the definition was last changed.
    pub updated_at: DateTime<Utc>,
    /// Names of the derived metrics the expression uses directly.
    pub uses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SavedDerivedMetric {
    /// True if the derived metric is new, false if it existed and now has
    /// this definition.
    pub created: bool,
    #[serde(flatten)]
    pub metric: DerivedMetric,
}

fn check_name(name: &str) -> ApiResult<()> {
    if name.trim().is_empty() || name != name.trim() || name.contains('`') {
        return Err(ApiError::invalid(
            "a derived metric name must not be empty, start or end with spaces, or contain '`'",
        ));
    }
    Ok(())
}

/// Creates a derived metric or changes its definition (FR-17). The
/// expression is validated first (FR-17a), together with the derived
/// metrics built on this one: the change takes effect in them too.
///
/// The check and the save are not one transaction. The database rules out
/// cycles by itself; two changes made at the same moment can otherwise
/// leave a metric that no longer fits the one it uses, which evaluation
/// reports per sample.
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
    let (_, created): (i64, bool) =
        sqlx::query_as("SELECT metric_id, created FROM save_derived_metric($1, $2, $3, $4, $5)")
            .bind(name)
            .bind(&req.expression)
            .bind(&req.unit)
            .bind(&req.description)
            .bind(&a.uses)
            .fetch_one(pool)
            .await?;
    Ok(SavedDerivedMetric {
        created,
        metric: get(pool, name).await?,
    })
}

type DerivedMetricRow = (
    i64,
    String,
    String,
    String,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
    Vec<String>,
);

/// All derived metrics, or the one called `name`; sorted by name.
async fn metrics_where(pool: &PgPool, name: Option<&str>) -> ApiResult<Vec<DerivedMetric>> {
    let rows: Vec<DerivedMetricRow> = sqlx::query_as(
        "SELECT m.id, m.name, m.expression, m.unit, m.description, m.created_at, m.updated_at, \
                ARRAY(SELECT u.name FROM derived_metric_dependency d \
                      JOIN derived_metric u ON u.id = d.uses_derived_metric_id \
                      WHERE d.derived_metric_id = m.id ORDER BY u.name) \
         FROM derived_metric m WHERE $1::text IS NULL OR m.name = $1 ORDER BY m.name",
    )
    .bind(name)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, name, expression, unit, description, created_at, updated_at, uses)| {
                DerivedMetric {
                    id,
                    name,
                    expression,
                    unit,
                    description,
                    created_at,
                    updated_at,
                    uses,
                }
            },
        )
        .collect())
}

pub async fn list(pool: &PgPool) -> ApiResult<Vec<DerivedMetric>> {
    metrics_where(pool, None).await
}

pub async fn get(pool: &PgPool, name: &str) -> ApiResult<DerivedMetric> {
    metrics_where(pool, Some(name))
        .await?
        .pop()
        .ok_or_else(|| ApiError::not_found(format!("derived metric {name} not found")))
}

// Renaming and deleting ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameDerivedMetric {
    /// The new name.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RenamedDerivedMetric {
    /// Derived metrics whose expressions were changed to the new name.
    pub rewritten: Vec<String>,
    #[serde(flatten)]
    pub metric: DerivedMetric,
}

/// Renames a derived metric (ADR 0017). The expressions of the derived
/// metrics that use it are changed to the new name in the same
/// transaction, so they keep working. Ad-hoc expressions elsewhere (e.g. in
/// an analysis view's URL) that use the old name do not.
pub async fn rename(
    pool: &PgPool,
    name: &str,
    req: &RenameDerivedMetric,
) -> ApiResult<RenamedDerivedMetric> {
    check_name(&req.name)?;
    if req.name == name {
        return Ok(RenamedDerivedMetric {
            rewritten: vec![],
            metric: get(pool, name).await?,
        });
    }
    let mut tx = pool.begin().await?;
    // Takes the lock that saves take too, so what uses the metric cannot
    // change until the commit.
    let id: Option<i64> = sqlx::query_scalar("SELECT rename_derived_metric($1, $2)")
        .bind(name)
        .bind(&req.name)
        .fetch_one(&mut *tx)
        .await?;
    let Some(id) = id else {
        return Err(ApiError::not_found(format!(
            "derived metric {name} not found"
        )));
    };
    let users: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT m.id, m.name, m.expression FROM derived_metric_dependency d \
         JOIN derived_metric m ON m.id = d.derived_metric_id \
         WHERE d.uses_derived_metric_id = $1 ORDER BY m.name",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let mut rewritten = Vec::new();
    for (user, user_name, expression) in users {
        let changed = rename_derived(&expression, name, &req.name).map_err(|d| {
            ApiError::invalid(format!(
                "derived metric {user_name} uses {name} but its expression does not parse \
                 ({}); change it before renaming",
                d.message
            ))
        })?;
        if let Some(changed) = changed {
            sqlx::query("UPDATE derived_metric SET expression = $2 WHERE id = $1")
                .bind(user)
                .bind(&changed)
                .execute(&mut *tx)
                .await?;
            rewritten.push(user_name);
        }
    }
    tx.commit().await?;
    Ok(RenamedDerivedMetric {
        rewritten,
        metric: get(pool, &req.name).await?,
    })
}

/// Deletes a derived metric (ADR 0017). A metric that other derived metrics
/// use cannot be deleted (409, `derived_metric_in_use`).
pub async fn delete(pool: &PgPool, name: &str) -> ApiResult<()> {
    let deleted: bool = sqlx::query_scalar("SELECT delete_derived_metric($1)")
        .bind(name)
        .fetch_one(pool)
        .await?;
    if !deleted {
        return Err(ApiError::not_found(format!(
            "derived metric {name} not found"
        )));
    }
    Ok(())
}

// Evaluation on one case execution -------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluateRequest {
    pub case_execution_id: Uuid,
    /// An ad-hoc expression. Give either this or `name`.
    pub expression: Option<String>,
    /// A saved derived metric.
    pub name: Option<String>,
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

/// Prepares a saved derived metric for evaluation, in its declared unit or
/// `unit`.
async fn prepare_saved(pool: &PgPool, name: &str, unit: Option<&str>) -> ApiResult<Prepared> {
    let library = load_library(pool, &BTreeSet::from([name.to_owned()])).await?;
    let Some(definition) = library.get(name) else {
        return Err(ApiError::not_found(format!(
            "derived metric {name} not found"
        )));
    };
    let expression = definition.expression.clone();
    let catalogue = catalog::unit_catalogue(pool).await?;
    let mut root = Root::definition(&library, name).map_err(ApiError::invalid)?;
    let (inputs, _) = root.closure(&library).map_err(ApiError::invalid)?;
    let names: BTreeSet<String> = inputs.metrics.union(&inputs.series).cloned().collect();
    let latest = LatestUnits::load(pool, &names).await?;
    if let Some(unit) = unit {
        root.unit = Some(unit.to_owned());
    }
    let compiled =
        compile(&library, &catalogue, &root, &latest).map_err(|d| invalid(&expression, &d))?;
    Ok(Prepared {
        signature: compiled.root.signature,
        library,
        catalogue,
        root,
        expression: format!("derived.{name}"),
    })
}

/// Evaluates an expression or a saved derived metric on every sample of a
/// case execution (warmup samples included).
pub async fn evaluate(pool: &PgPool, req: &EvaluateRequest) -> ApiResult<EvaluateResponse> {
    let prepared = match (&req.expression, &req.name) {
        (Some(e), None) => prepare(pool, e, req.unit.as_deref()).await?,
        (None, Some(name)) => prepare_saved(pool, name, req.unit.as_deref()).await?,
        _ => return Err(ApiError::bad_request("give either expression or name")),
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
