//! Run lifecycle operations of the web UI and the stale-run job.
//!
//! Invalidation (FR-9) and hard delete (FR-10) are only reachable from the
//! UI port, never with an API token (NFR-2).

use std::time::Duration;

use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InvalidateRun {
    /// Why the run is invalid.
    pub reason: String,
}

/// Marks a run invalid (FR-9). Invalid runs stay stored and are excluded
/// from analysis by default.
pub async fn invalidate(pool: &PgPool, run_id: Uuid, req: &InvalidateRun) -> ApiResult<()> {
    let reason = req.reason.trim();
    if reason.is_empty() {
        return Err(ApiError::invalid(
            "a reason is required to mark a run invalid",
        ));
    }
    set_validity(pool, run_id, false, Some(reason)).await
}

/// Marks a run valid again (FR-9).
pub async fn validate(pool: &PgPool, run_id: Uuid) -> ApiResult<()> {
    set_validity(pool, run_id, true, None).await
}

async fn set_validity(
    pool: &PgPool,
    run_id: Uuid,
    valid: bool,
    reason: Option<&str>,
) -> ApiResult<()> {
    let updated = sqlx::query("UPDATE run SET is_valid = $2, invalid_reason = $3 WHERE id = $1")
        .bind(run_id)
        .bind(valid)
        .bind(reason)
        .execute(pool)
        .await?
        .rows_affected();
    if updated == 0 {
        return Err(ApiError::not_found(format!("run {run_id} not found")));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteRun {
    /// Must repeat the run id to confirm the deletion.
    pub confirm_run_id: String,
}

/// Hard-deletes a run with all its results and attachments (FR-10): the
/// rows through `delete_run()`, then the attachment files (ADR 0003).
pub async fn delete(state: &AppState, run_id: Uuid, req: &DeleteRun) -> ApiResult<()> {
    if req.confirm_run_id.trim() != run_id.to_string() {
        return Err(ApiError::bad_request(
            "confirm_run_id does not match the run id",
        ));
    }
    let found: bool = sqlx::query_scalar("SELECT delete_run($1)")
        .bind(run_id)
        .fetch_one(&state.pool)
        .await?;
    if !found {
        return Err(ApiError::not_found(format!("run {run_id} not found")));
    }
    tracing::warn!(%run_id, "run hard-deleted");
    state
        .attachments
        .delete_run(run_id)
        .await
        .map_err(|e| anyhow::anyhow!("run {run_id} deleted, but removing its files failed: {e}"))?;
    Ok(())
}

/// Advisory lock key of the stale-run job, so only one replica runs it at a
/// time.
const STALE_RUN_LOCK: i64 = 4_711_202_612;

/// Aborts running runs without uploads for longer than `timeout` (FR-2),
/// unless another replica is doing it right now. Returns the aborted runs.
pub async fn abort_stale_runs(pool: &PgPool, timeout: Duration) -> ApiResult<Vec<Uuid>> {
    let mut tx = pool.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(STALE_RUN_LOCK)
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Ok(vec![]);
    }
    let timeout = format!("{} milliseconds", timeout.as_millis());
    let aborted: Vec<Uuid> = sqlx::query_scalar("SELECT abort_stale_runs($1::interval)")
        .bind(timeout)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(aborted)
}
