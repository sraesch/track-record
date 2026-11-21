//! Definitions and dimensions: suites, scenarios, metrics, units, client
//! devices, environments, SUT versions, assets, test cases and aliases.
//! All are versioned or deduplicated and created only through the schema's
//! `ensure_*` / `set_alias` functions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};

use crate::db::{MetricDirection, MetricValueType};
use crate::error::{ApiError, ApiResult};
use crate::service::query::{Aliases, DEVICE_DIMENSION};
use crate::service::runs::{SutVersion, TestCaseSummary, test_case_assets};

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct DefinitionVersion {
    pub id: i64,
    pub version: i32,
    #[schema(value_type = Object)]
    pub definition: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionedDefinition {
    pub id: i64,
    pub name: String,
    pub versions: Vec<DefinitionVersion>,
}

async fn versioned(pool: &PgPool, parent: &str) -> ApiResult<Vec<VersionedDefinition>> {
    let rows: Vec<(i64, String, i64, i32, Value, DateTime<Utc>)> = sqlx::query_as(&format!(
        "SELECT p.id, p.name, v.id, v.version, v.definition, v.created_at \
         FROM {parent} p JOIN {parent}_version v ON v.{parent}_id = p.id \
         ORDER BY p.name, v.version"
    ))
    .fetch_all(pool)
    .await?;
    let mut out: Vec<VersionedDefinition> = Vec::new();
    for (id, name, vid, version, definition, created_at) in rows {
        if out.last().is_none_or(|d| d.id != id) {
            out.push(VersionedDefinition {
                id,
                name,
                versions: vec![],
            });
        }
        out.last_mut().unwrap().versions.push(DefinitionVersion {
            id: vid,
            version,
            definition,
            created_at,
        });
    }
    Ok(out)
}

pub async fn suites(pool: &PgPool) -> ApiResult<Vec<VersionedDefinition>> {
    versioned(pool, "suite").await
}

pub async fn scenarios(pool: &PgPool) -> ApiResult<Vec<VersionedDefinition>> {
    versioned(pool, "scenario").await
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricVersion {
    pub id: i64,
    pub version: i32,
    pub unit: String,
    pub value_type: MetricValueType,
    pub direction: MetricDirection,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Metric {
    pub id: i64,
    pub name: String,
    pub versions: Vec<MetricVersion>,
}

pub async fn metrics(pool: &PgPool) -> ApiResult<Vec<Metric>> {
    let rows: Vec<(
        i64,
        String,
        i64,
        i32,
        String,
        MetricValueType,
        MetricDirection,
        String,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT m.id, m.name, v.id, v.version, v.unit, v.value_type, v.direction, \
                    v.description, v.created_at \
             FROM metric m JOIN metric_version v ON v.metric_id = m.id ORDER BY m.name, v.version",
    )
    .fetch_all(pool)
    .await?;
    let mut out: Vec<Metric> = Vec::new();
    for (id, name, vid, version, unit, value_type, direction, description, created_at) in rows {
        if out.last().is_none_or(|m| m.id != id) {
            out.push(Metric {
                id,
                name,
                versions: vec![],
            });
        }
        out.last_mut().unwrap().versions.push(MetricVersion {
            id: vid,
            version,
            unit,
            value_type,
            direction,
            description,
            created_at,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Unit {
    pub symbol: String,
    pub quantity: String,
    /// Value in this unit times `to_base` is the value in the base unit of
    /// the quantity.
    pub to_base: f64,
    pub name: String,
}

pub async fn units(pool: &PgPool) -> ApiResult<Vec<Unit>> {
    Ok(sqlx::query_as(
        "SELECT symbol, quantity, to_base, name FROM unit ORDER BY quantity, to_base",
    )
    .fetch_all(pool)
    .await?)
}

/// The unit catalogue for conversions (FR-13a).
pub async fn unit_catalogue(pool: &PgPool) -> ApiResult<track_record_core::units::UnitCatalogue> {
    Ok(track_record_core::units::UnitCatalogue::new(
        units(pool)
            .await?
            .into_iter()
            .map(|u| track_record_core::units::CatalogueUnit {
                symbol: u.symbol,
                quantity: u.quantity,
                to_base: u.to_base,
                name: u.name,
            }),
    ))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ClientDevice {
    pub id: i64,
    pub name: String,
    /// Name after aliases.
    pub resolved_name: String,
    pub created_at: DateTime<Utc>,
}

pub async fn client_devices(pool: &PgPool) -> ApiResult<Vec<ClientDevice>> {
    let aliases = Aliases::load(pool).await?;
    let rows: Vec<(i64, String, DateTime<Utc>)> =
        sqlx::query_as("SELECT id, name, created_at FROM client_device ORDER BY name")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, created_at)| ClientDevice {
            id,
            resolved_name: aliases.resolve_device(&name),
            name,
            created_at,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Environment {
    pub id: i64,
    #[schema(value_type = Object)]
    pub details: Value,
    /// SHA-256 of the details, hex.
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
}

pub async fn environment(pool: &PgPool, id: i64) -> ApiResult<Environment> {
    sqlx::query_as(
        "SELECT id, details, encode(fingerprint, 'hex') AS fingerprint, created_at \
         FROM environment WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found(format!("environment {id} not found")))
}

pub async fn sut_versions(pool: &PgPool) -> ApiResult<Vec<SutVersion>> {
    Ok(sqlx::query_as(
        "SELECT id, commit, branch, build_config, tag FROM sut_version ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub version: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn assets(pool: &PgPool) -> ApiResult<Vec<Asset>> {
    Ok(sqlx::query_as(
        "SELECT id, name, version, content_hash, created_at FROM asset ORDER BY name, id",
    )
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct TestCaseQuery {
    /// Scenario name.
    pub scenario: Option<String>,
}

pub async fn test_cases(pool: &PgPool, query: &TestCaseQuery) -> ApiResult<Vec<TestCaseSummary>> {
    test_cases_where(pool, query.scenario.as_deref(), None).await
}

pub async fn test_case(pool: &PgPool, id: i64) -> ApiResult<TestCaseSummary> {
    test_cases_where(pool, None, Some(id))
        .await?
        .pop()
        .ok_or_else(|| ApiError::not_found(format!("test case {id} not found")))
}

async fn test_cases_where(
    pool: &PgPool,
    scenario: Option<&str>,
    id: Option<i64>,
) -> ApiResult<Vec<TestCaseSummary>> {
    let aliases = Aliases::load(pool).await?;
    let rows: Vec<(i64, String, i32, i64, Value, Value, Vec<String>)> = sqlx::query_as(
        "SELECT tc.id, sc.name, scv.version, scv.id, tc.parameters, tc.parameter_units, \
                ARRAY(SELECT t.tag FROM test_case_tag t WHERE t.test_case_id = tc.id ORDER BY t.tag) \
         FROM test_case tc JOIN scenario_version scv ON scv.id = tc.scenario_version_id \
         JOIN scenario sc ON sc.id = scv.scenario_id \
         WHERE ($1::text IS NULL OR sc.name = $1) AND ($2::bigint IS NULL OR tc.id = $2) \
         ORDER BY sc.name, scv.version, tc.id",
    )
    .bind(scenario)
    .bind(id)
    .fetch_all(pool)
    .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let assets = test_case_assets(pool, &ids).await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                scenario,
                scenario_version,
                scenario_version_id,
                parameters,
                parameter_units,
                tags,
            )| {
                TestCaseSummary {
                    id,
                    scenario,
                    scenario_version,
                    scenario_version_id,
                    resolved_parameters: aliases.resolve_parameters(&parameters),
                    parameters,
                    parameter_units,
                    assets: assets.get(&id).cloned().unwrap_or_default(),
                    tags,
                }
            },
        )
        .collect())
}

// Aliases (FR-14) ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Alias {
    pub id: i64,
    /// `client_device` or `parameter:<name>`.
    pub dimension: String,
    #[schema(value_type = Object)]
    pub source: Value,
    /// Current target; `null` if the alias was removed.
    #[schema(value_type = Object)]
    pub target: Option<Value>,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

pub async fn aliases(pool: &PgPool) -> ApiResult<Vec<Alias>> {
    Ok(sqlx::query_as(
        "SELECT a.id, a.dimension, a.source_value AS source, v.target_value AS target, v.version, \
                v.created_at AS updated_at \
         FROM alias a JOIN LATERAL ( \
             SELECT target_value, version, created_at FROM alias_version \
             WHERE alias_id = a.id ORDER BY version DESC LIMIT 1) v ON true \
         ORDER BY a.dimension, a.source_value",
    )
    .fetch_all(pool)
    .await?)
}

/// Sets or (with `target: null`) removes an alias. Every change is a new
/// alias version (FR-7).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetAlias {
    /// `client_device` or `parameter:<name>`.
    pub dimension: String,
    /// JSON scalar; a device name is a string.
    #[schema(value_type = Object)]
    pub source: Value,
    /// JSON scalar, or `null` to remove the alias.
    #[schema(value_type = Object)]
    pub target: Option<Value>,
}

fn validate_alias(req: &SetAlias) -> ApiResult<()> {
    let dimension = req.dimension.trim();
    let valid_dimension = dimension == DEVICE_DIMENSION
        || dimension
            .strip_prefix("parameter:")
            .is_some_and(|p| !p.trim().is_empty());
    if !valid_dimension {
        return Err(ApiError::invalid(
            "dimension must be client_device or parameter:<name>",
        ));
    }
    let scalar = |v: &Value| v.is_number() || v.is_string() || v.is_boolean();
    if !scalar(&req.source) || req.target.as_ref().is_some_and(|t| !scalar(t)) {
        return Err(ApiError::invalid(
            "alias values must be JSON numbers, strings or booleans",
        ));
    }
    if dimension == DEVICE_DIMENSION
        && (!req.source.is_string() || req.target.as_ref().is_some_and(|t| !t.is_string()))
    {
        return Err(ApiError::invalid(
            "device aliases map names (strings) to names",
        ));
    }
    Ok(())
}

pub async fn set_alias(pool: &PgPool, req: &SetAlias) -> ApiResult<Option<Alias>> {
    validate_alias(req)?;
    let dimension = req.dimension.trim();
    let version_id: Option<i64> = sqlx::query_scalar("SELECT set_alias($1, $2, $3)")
        .bind(dimension)
        .bind(&req.source)
        .bind(&req.target)
        .fetch_one(pool)
        .await?;
    if version_id.is_none() {
        return Ok(None);
    }
    Ok(aliases(pool)
        .await?
        .into_iter()
        .find(|a| a.dimension == dimension && a.source == req.source))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn alias(dimension: &str, source: Value, target: Option<Value>) -> SetAlias {
        SetAlias {
            dimension: dimension.into(),
            source,
            target,
        }
    }

    #[test]
    fn alias_validation() {
        assert!(validate_alias(&alias("client_device", json!("a"), Some(json!("b")))).is_ok());
        assert!(validate_alias(&alias("client_device", json!("a"), None)).is_ok());
        assert!(validate_alias(&alias("parameter:size", json!(10), Some(json!(20)))).is_ok());
        assert!(validate_alias(&alias("client_device", json!(1), Some(json!("b")))).is_err());
        assert!(validate_alias(&alias("parameter:", json!(1), Some(json!(2)))).is_err());
        assert!(validate_alias(&alias("suite", json!("a"), Some(json!("b")))).is_err());
        assert!(validate_alias(&alias("parameter:x", json!([1]), Some(json!(2)))).is_err());
    }
}
