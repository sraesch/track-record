//! track-record-server
//!
//!   track-record-server [serve]         run the service (configuration from the environment)
//!   track-record-server openapi machine print the machine API's OpenAPI document
//!   track-record-server openapi ui      print the UI API's OpenAPI document

use anyhow::{Context, bail};
use tracing_subscriber::EnvFilter;

use track_record_server::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["serve"] => serve().await,
        ["openapi", which] => {
            println!(
                "{}",
                track_record_server::openapi_json(which).context("unknown API")?
            );
            Ok(())
        }
        _ => bail!("usage: track-record-server [serve | openapi machine|ui]"),
    }
}

async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();
    let config = Config::from_env()?;
    track_record_server::run(config, shutdown_signal()).await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("shutting down");
}
