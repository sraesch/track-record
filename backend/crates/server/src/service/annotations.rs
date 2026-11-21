//! Tags and annotations. Unlike results these are editable, through the web
//! UI and with an `annotate` token (REST and MCP). They have no author
//! (NFR-2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

fn clean_tag(tag: &str) -> ApiResult<String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err(ApiError::invalid("tag must not be empty"));
    }
    Ok(tag.to_owned())
}

async fn require_run(pool: &PgPool, run_id: Uuid) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM run WHERE id = $1)")
        .bind(run_id)
        .fetch_one(pool)
        .await?;
    exists
        .then_some(())
        .ok_or_else(|| ApiError::not_found(format!("run {run_id} not found")))
}

async fn require_test_case(pool: &PgPool, id: i64) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM test_case WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await?;
    exists
        .then_some(())
        .ok_or_else(|| ApiError::not_found(format!("test case {id} not found")))
}

/// Adds a tag to a run. Adding an existing tag is a no-op.
pub async fn add_run_tag(pool: &PgPool, run_id: Uuid, tag: &str) -> ApiResult<()> {
    let tag = clean_tag(tag)?;
    require_run(pool, run_id).await?;
    sqlx::query("INSERT INTO run_tag (run_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(run_id)
        .bind(tag)
        .execute(pool)
        .await?;
    Ok(())
}

/// Removes a tag from a run. Removing a missing tag is a no-op.
pub async fn remove_run_tag(pool: &PgPool, run_id: Uuid, tag: &str) -> ApiResult<()> {
    require_run(pool, run_id).await?;
    sqlx::query("DELETE FROM run_tag WHERE run_id = $1 AND tag = $2")
        .bind(run_id)
        .bind(tag.trim())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_test_case_tag(pool: &PgPool, test_case_id: i64, tag: &str) -> ApiResult<()> {
    let tag = clean_tag(tag)?;
    require_test_case(pool, test_case_id).await?;
    sqlx::query(
        "INSERT INTO test_case_tag (test_case_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(test_case_id)
    .bind(tag)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_test_case_tag(pool: &PgPool, test_case_id: i64, tag: &str) -> ApiResult<()> {
    require_test_case(pool, test_case_id).await?;
    sqlx::query("DELETE FROM test_case_tag WHERE test_case_id = $1 AND tag = $2")
        .bind(test_case_id)
        .bind(tag.trim())
        .execute(pool)
        .await?;
    Ok(())
}

/// A free-text note on exactly one of: a run, a test case or a point in
/// time.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Annotation {
    pub id: Uuid,
    pub run_id: Option<Uuid>,
    pub test_case_id: Option<i64>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const ANNOTATION_SELECT: &str =
    "SELECT id, run_id, test_case_id, occurred_at, body, created_at, updated_at FROM annotation";

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAnnotation {
    pub run_id: Option<Uuid>,
    pub test_case_id: Option<i64>,
    /// A point in time, e.g. "switched to new compression".
    pub occurred_at: Option<DateTime<Utc>>,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateAnnotation {
    pub body: String,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct AnnotationQuery {
    pub run_id: Option<Uuid>,
    pub test_case_id: Option<i64>,
    /// Point-in-time annotations at or after this time.
    pub from: Option<DateTime<Utc>>,
    /// Point-in-time annotations before this time.
    pub to: Option<DateTime<Utc>>,
}

fn clean_body(body: &str) -> ApiResult<&str> {
    let body = body.trim();
    if body.is_empty() {
        return Err(ApiError::invalid("annotation body must not be empty"));
    }
    Ok(body)
}

pub async fn create(pool: &PgPool, req: &CreateAnnotation) -> ApiResult<Annotation> {
    let body = clean_body(&req.body)?;
    let targets = [
        req.run_id.is_some(),
        req.test_case_id.is_some(),
        req.occurred_at.is_some(),
    ];
    if targets.iter().filter(|t| **t).count() != 1 {
        return Err(ApiError::invalid(
            "an annotation needs exactly one of run_id, test_case_id and occurred_at",
        ));
    }
    if let Some(run_id) = req.run_id {
        require_run(pool, run_id).await?;
    }
    if let Some(id) = req.test_case_id {
        require_test_case(pool, id).await?;
    }
    Ok(sqlx::query_as(
        "INSERT INTO annotation (run_id, test_case_id, occurred_at, body) VALUES ($1, $2, $3, $4) \
         RETURNING id, run_id, test_case_id, occurred_at, body, created_at, updated_at",
    )
    .bind(req.run_id)
    .bind(req.test_case_id)
    .bind(req.occurred_at)
    .bind(body)
    .fetch_one(pool)
    .await?)
}

pub async fn update(pool: &PgPool, id: Uuid, req: &UpdateAnnotation) -> ApiResult<Annotation> {
    let body = clean_body(&req.body)?;
    sqlx::query_as(
        "UPDATE annotation SET body = $2 WHERE id = $1 \
         RETURNING id, run_id, test_case_id, occurred_at, body, created_at, updated_at",
    )
    .bind(id)
    .bind(body)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found(format!("annotation {id} not found")))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> ApiResult<()> {
    let deleted = sqlx::query("DELETE FROM annotation WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(ApiError::not_found(format!("annotation {id} not found")));
    }
    Ok(())
}

pub async fn get(pool: &PgPool, id: Uuid) -> ApiResult<Annotation> {
    sqlx::query_as(&format!("{ANNOTATION_SELECT} WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("annotation {id} not found")))
}

pub async fn list(pool: &PgPool, query: &AnnotationQuery) -> ApiResult<Vec<Annotation>> {
    let mut q = QueryBuilder::<Postgres>::new(ANNOTATION_SELECT);
    q.push(" WHERE true");
    if let Some(run_id) = query.run_id {
        q.push(" AND run_id = ").push_bind(run_id);
    }
    if let Some(id) = query.test_case_id {
        q.push(" AND test_case_id = ").push_bind(id);
    }
    if let Some(from) = query.from {
        q.push(" AND occurred_at >= ").push_bind(from);
    }
    if let Some(to) = query.to {
        q.push(" AND occurred_at < ").push_bind(to);
    }
    q.push(" ORDER BY coalesce(occurred_at, created_at), id");
    Ok(q.build_query_as().fetch_all(pool).await?)
}

pub async fn for_run(pool: &PgPool, run_id: Uuid) -> ApiResult<Vec<Annotation>> {
    list(
        pool,
        &AnnotationQuery {
            run_id: Some(run_id),
            ..Default::default()
        },
    )
    .await
}
