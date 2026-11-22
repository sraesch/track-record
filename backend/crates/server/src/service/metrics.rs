//! Uploaded metrics (FR-7, ADR 0018).
//!
//! A metric has one definition: unit, value type, direction and
//! description. There are no versions. Uploads create metrics that do not
//! exist yet and never change one (`service::ingest`). Changing, renaming
//! and deleting happen only in the web UI:
//!
//! - A change applies to every result of the metric, historical ones
//!   included. Stored values are plain numbers and are not converted: with
//!   another unit they are read in that unit.
//! - A change of the unit also reaches the derived metrics that use the
//!   metric. One that would make such a derived metric invalid is rejected.
//! - Renaming rewrites the expressions of the derived metrics that use the
//!   metric. Uploads identify a metric by its name, so the harness has to
//!   send the new name from then on.
//! - Only a metric without results that no derived metric uses can be
//!   deleted (FR-8).

use std::collections::HashMap;

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use track_record_core::dsl::{MAX_SOURCE_LEN, rename_metric};
use utoipa::ToSchema;

use crate::db::{MetricDirection, MetricValueType};
use crate::error::{ApiError, ApiResult};
use crate::service::derived;

/// An uploaded metric. It has one definition, which can be changed in the
/// web UI.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Metric {
    pub id: i64,
    pub name: String,
    /// Unit symbol; `""` for dimensionless (FR-13a).
    pub unit: String,
    pub value_type: MetricValueType,
    pub direction: MetricDirection,
    pub description: String,
    pub created_at: DateTime<Utc>,
    /// When the definition was last changed.
    pub updated_at: DateTime<Utc>,
    /// True if measurements or series of the metric are stored. Such a
    /// metric cannot be deleted.
    pub has_results: bool,
    /// Names of the derived metrics whose expressions refer to the metric.
    pub used_by: Vec<String>,
}

type MetricRow = (
    i64,
    String,
    String,
    MetricValueType,
    MetricDirection,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
    bool,
);

/// All metrics, or the one called `name`; sorted by name.
async fn metrics_where(pool: &PgPool, name: Option<&str>) -> ApiResult<Vec<Metric>> {
    let rows: Vec<MetricRow> = sqlx::query_as(
        "SELECT m.id, m.name, m.unit, m.value_type, m.direction, m.description, m.created_at, \
                m.updated_at, \
                EXISTS (SELECT 1 FROM measurement r WHERE r.metric_id = m.id) \
                    OR EXISTS (SELECT 1 FROM series s WHERE s.metric_id = m.id) \
         FROM metric m WHERE $1::text IS NULL OR m.name = $1 ORDER BY m.name",
    )
    .bind(name)
    .fetch_all(pool)
    .await?;
    let mut users: HashMap<String, Vec<String>> = if rows.is_empty() {
        HashMap::new()
    } else {
        derived::metric_users(pool).await?
    };
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                unit,
                value_type,
                direction,
                description,
                created_at,
                updated_at,
                has_results,
            )| Metric {
                id,
                used_by: users.remove(&name).unwrap_or_default(),
                name,
                unit,
                value_type,
                direction,
                description,
                created_at,
                updated_at,
                has_results,
            },
        )
        .collect())
}

pub async fn list(pool: &PgPool) -> ApiResult<Vec<Metric>> {
    metrics_where(pool, None).await
}

pub async fn get(pool: &PgPool, name: &str) -> ApiResult<Metric> {
    metrics_where(pool, Some(name))
        .await?
        .pop()
        .ok_or_else(|| not_found(name))
}

fn not_found(name: &str) -> ApiError {
    ApiError::not_found(format!("metric {name} not found"))
}

// Changing -------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveMetric {
    /// Unit symbol; `""` for dimensionless. Stored values are not converted.
    pub unit: String,
    pub value_type: MetricValueType,
    pub direction: MetricDirection,
    #[serde(default)]
    pub description: String,
}

/// Changes the definition of a metric (ADR 0018). The change applies to
/// every result of the metric: stored values are read in the new unit, they
/// are not converted. A unit that would make a derived metric built on the
/// metric invalid is rejected (422, `breaks_derived_metrics`).
///
/// Like for derived metrics, the check and the save are not one
/// transaction; evaluation checks units again per sample.
pub async fn save(pool: &PgPool, name: &str, req: &SaveMetric) -> ApiResult<Metric> {
    if req.unit != req.unit.trim() {
        return Err(ApiError::invalid(
            "a unit must not start or end with spaces",
        ));
    }
    let current = get(pool, name).await?;
    if current.unit != req.unit {
        let broken = derived::broken_by_metric_unit(pool, name, &req.unit).await?;
        if !broken.is_empty() {
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "breaks_derived_metrics",
                broken.join("; "),
            ));
        }
    }
    let id: Option<i64> = sqlx::query_scalar("SELECT save_metric($1, $2, $3, $4, $5)")
        .bind(name)
        .bind(&req.unit)
        .bind(req.value_type)
        .bind(req.direction)
        .bind(&req.description)
        .fetch_one(pool)
        .await?;
    if id.is_none() {
        return Err(not_found(name));
    }
    get(pool, name).await
}

// Renaming and deleting ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameMetric {
    /// The new name.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RenamedMetric {
    /// Derived metrics whose expressions were changed to the new name.
    pub rewritten: Vec<String>,
    #[serde(flatten)]
    pub metric: Metric,
}

fn check_name(name: &str) -> ApiResult<()> {
    if name.trim().is_empty() || name != name.trim() || name.contains('`') {
        return Err(ApiError::invalid(
            "a metric name must not be empty, start or end with spaces, or contain '`'",
        ));
    }
    Ok(())
}

/// Renames a metric (ADR 0018). Its results stay with it. The expressions
/// of the derived metrics that use it are changed to the new name in the
/// same transaction. Ad-hoc expressions elsewhere (e.g. in an analysis
/// view's URL) that use the old name stop working, and an upload under the
/// old name creates a new metric.
pub async fn rename(pool: &PgPool, name: &str, req: &RenameMetric) -> ApiResult<RenamedMetric> {
    check_name(&req.name)?;
    if req.name == name {
        return Ok(RenamedMetric {
            rewritten: vec![],
            metric: get(pool, name).await?,
        });
    }
    let mut tx = pool.begin().await?;
    // Takes the lock that saves of derived metrics take, so their
    // expressions cannot change until the commit.
    let id: Option<i64> = sqlx::query_scalar("SELECT rename_metric($1, $2)")
        .bind(name)
        .bind(&req.name)
        .fetch_one(&mut *tx)
        .await?;
    if id.is_none() {
        return Err(not_found(name));
    }
    let mut rewritten = Vec::new();
    for (user, user_name, expression) in derived::expressions(&mut *tx).await? {
        // An expression that does not parse refers to nothing.
        let Ok(Some(changed)) = rename_metric(&expression, name, &req.name) else {
            continue;
        };
        if changed.len() > MAX_SOURCE_LEN {
            return Err(ApiError::invalid(format!(
                "the expression of derived metric {user_name} would become longer than \
                 {MAX_SOURCE_LEN} bytes; shorten it or choose a shorter name"
            )));
        }
        sqlx::query("UPDATE derived_metric SET expression = $2 WHERE id = $1")
            .bind(user)
            .bind(&changed)
            .execute(&mut *tx)
            .await?;
        rewritten.push(user_name);
    }
    tx.commit().await?;
    Ok(RenamedMetric {
        rewritten,
        metric: get(pool, &req.name).await?,
    })
}

/// Deletes a metric (ADR 0018). Not possible while it has results (they are
/// deleted only with their run, FR-8) or while derived metrics use it (409,
/// `metric_in_use`).
pub async fn delete(pool: &PgPool, name: &str) -> ApiResult<()> {
    let mut tx = pool.begin().await?;
    // The lock that saves of derived metrics take: what uses the metric
    // cannot change until the commit.
    sqlx::query("LOCK TABLE derived_metric IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM metric WHERE name = $1)")
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(not_found(name));
    }
    let users = derived::users_of_metric(&derived::expressions(&mut *tx).await?, name);
    if !users.is_empty() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "metric_in_use",
            format!(
                "metric {name} is used by the derived metrics {}; change or delete those first",
                users.join(", ")
            ),
        ));
    }
    let deleted: bool = sqlx::query_scalar("SELECT delete_metric($1)")
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
    if !deleted {
        return Err(not_found(name));
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_expressions_cannot_refer_to_are_rejected() {
        assert!(check_name("load_time").is_ok());
        assert!(check_name("load time (cold)").is_ok());
        assert!(check_name("").is_err());
        assert!(check_name(" load_time").is_err());
        assert!(check_name("load`time").is_err());
    }
}
