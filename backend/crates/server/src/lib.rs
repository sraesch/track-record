//! track-record backend: REST API on two listeners (machine and UI), the MCP
//! server and background jobs. See docs/adr/0008-backend-service.md.

pub mod api;
pub mod attachment_store;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod jobs;
pub mod series_store;
pub mod service;

use std::future::Future;
use std::sync::Arc;

use anyhow::Context;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::attachment_store::AttachmentStore;
use crate::config::Config;

/// Shared state of all handlers. Holds no request state: the backend is
/// stateless and scales horizontally (NFR-1).
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub attachments: Arc<AttachmentStore>,
    pub max_body_bytes: usize,
}

impl AppState {
    pub fn new(pool: PgPool, config: &Config) -> Self {
        Self {
            pool,
            attachments: Arc::new(AttachmentStore::new(&config.attachment_root)),
            max_body_bytes: config.max_body_bytes,
        }
    }
}

/// Runs the service until `shutdown` completes: checks the schema, then
/// serves both listeners and runs the background jobs.
pub async fn run(
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let pool = db::connect(&config.database_url, config.database_max_connections).await?;
    db::check_schema(&pool).await?;
    let state = AppState::new(pool.clone(), &config);
    state.attachments.init().await.with_context(|| {
        format!(
            "attachment root {} is not writable",
            config.attachment_root.display()
        )
    })?;

    let machine = TcpListener::bind(config.machine_addr)
        .await
        .with_context(|| format!("cannot listen on {}", config.machine_addr))?;
    let ui = TcpListener::bind(config.ui_addr)
        .await
        .with_context(|| format!("cannot listen on {}", config.ui_addr))?;
    tracing::info!(machine = %config.machine_addr, ui = %config.ui_addr, "listening");

    let cancel = CancellationToken::new();
    let job = tokio::spawn(jobs::stale_run_loop(
        pool,
        config.stale_run_timeout,
        config.stale_run_check_interval,
        cancel.clone(),
    ));
    let trigger = cancel.clone();
    tokio::spawn(async move {
        shutdown.await;
        trigger.cancel();
    });
    serve(state, machine, ui, cancel.clone()).await?;
    cancel.cancel();
    let _ = job.await;
    Ok(())
}

/// Serves both listeners until `cancel` is triggered.
pub async fn serve(
    state: AppState,
    machine: TcpListener,
    ui: TcpListener,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let trace = || tower_http::trace::TraceLayer::new_for_http();
    let machine_app = api::machine_router(state.clone()).layer(trace());
    let ui_app = api::ui_router(state).layer(trace());
    let (c1, c2) = (cancel.clone(), cancel.clone());
    let machine = axum::serve(machine, machine_app)
        .with_graceful_shutdown(async move { c1.cancelled().await });
    let ui = axum::serve(ui, ui_app).with_graceful_shutdown(async move { c2.cancelled().await });
    tokio::try_join!(machine, ui)?;
    Ok(())
}

/// The OpenAPI document of `machine` or `ui` as pretty JSON.
pub fn openapi_json(which: &str) -> Option<String> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .expect("lazy pool");
    let state = AppState {
        pool,
        attachments: Arc::new(AttachmentStore::new("/nonexistent")),
        max_body_bytes: 0,
    };
    let doc = match which {
        "machine" => api::machine_api(&state).1,
        "ui" => api::ui_api(&state).1,
        _ => return None,
    };
    Some(doc.to_pretty_json().expect("OpenAPI document serializes"))
}
