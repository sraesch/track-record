//! Background jobs. Every replica runs them; an advisory lock keeps one
//! replica at a time doing the work, so the backend stays stateless.

use std::time::Duration;

use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::service::lifecycle;

/// Aborts stale running runs (FR-2) every `interval` until cancelled.
pub async fn stale_run_loop(
    pool: PgPool,
    timeout: Duration,
    interval: Duration,
    cancel: CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = ticker.tick() => {}
        }
        match lifecycle::abort_stale_runs(&pool, timeout).await {
            Ok(aborted) => {
                for run_id in aborted {
                    tracing::info!(%run_id, ?timeout, "aborted stale run: no upload within the timeout");
                }
            }
            Err(err) => tracing::error!(error = %err, "stale run check failed"),
        }
    }
}
