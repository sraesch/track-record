//! Configuration from environment variables.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, bail};

#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection URL (`DATABASE_URL`).
    pub database_url: String,
    pub database_max_connections: u32,
    /// Listener for machine clients: `/api` and `/mcp` with API tokens.
    pub machine_addr: SocketAddr,
    /// Listener for the web UI behind the auth proxy: no tokens, includes
    /// invalidation, deletion and token management.
    pub ui_addr: SocketAddr,
    /// Root of the shared attachment volume (ADR 0003).
    pub attachment_root: PathBuf,
    /// A running run without uploads for this long is aborted (FR-2).
    pub stale_run_timeout: Duration,
    pub stale_run_check_interval: Duration,
    /// Maximum size of a JSON request body.
    pub max_body_bytes: usize,
    /// `Host` header values accepted on `/mcp` (`TR_MCP_ALLOWED_HOSTS`,
    /// comma-separated, `host` or `host:port`). Empty disables the check;
    /// the API token already keeps browsers out.
    pub mcp_allowed_hosts: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let database_url = get("DATABASE_URL").context("DATABASE_URL is not set")?;
        let parse_addr = |key: &str, default: &str| -> anyhow::Result<SocketAddr> {
            let value = get(key).unwrap_or_else(|| default.to_owned());
            value
                .parse()
                .with_context(|| format!("{key}: invalid address {value:?}"))
        };
        let parse_duration = |key: &str, default: &str| -> anyhow::Result<Duration> {
            let value = get(key).unwrap_or_else(|| default.to_owned());
            humantime::parse_duration(&value)
                .with_context(|| format!("{key}: invalid duration {value:?}"))
        };
        let parse_number = |key: &str, default: usize| -> anyhow::Result<usize> {
            get(key).map_or(Ok(default), |v| {
                v.parse()
                    .with_context(|| format!("{key}: invalid number {v:?}"))
            })
        };

        let config = Self {
            database_url,
            database_max_connections: parse_number("TR_DATABASE_MAX_CONNECTIONS", 16)? as u32,
            machine_addr: parse_addr("TR_MACHINE_ADDR", "0.0.0.0:8080")?,
            ui_addr: parse_addr("TR_UI_ADDR", "0.0.0.0:8081")?,
            attachment_root: get("TR_ATTACHMENT_ROOT")
                .map(PathBuf::from)
                .context("TR_ATTACHMENT_ROOT is not set")?,
            stale_run_timeout: parse_duration("TR_STALE_RUN_TIMEOUT", "1h")?,
            stale_run_check_interval: parse_duration("TR_STALE_RUN_CHECK_INTERVAL", "1m")?,
            max_body_bytes: parse_number("TR_MAX_BODY_BYTES", 256 * 1024 * 1024)?,
            mcp_allowed_hosts: get("TR_MCP_ALLOWED_HOSTS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|h| !h.is_empty())
                .map(str::to_owned)
                .collect(),
        };
        if config.machine_addr == config.ui_addr {
            bail!("TR_MACHINE_ADDR and TR_UI_ADDR must differ");
        }
        if config.stale_run_timeout.is_zero() || config.stale_run_check_interval.is_zero() {
            bail!("TR_STALE_RUN_TIMEOUT and TR_STALE_RUN_CHECK_INTERVAL must be positive");
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn defaults() {
        let c = Config::from_lookup(lookup(&[
            ("DATABASE_URL", "postgres://localhost/track_record"),
            ("TR_ATTACHMENT_ROOT", "/data"),
        ]))
        .unwrap();
        assert_eq!(c.machine_addr.port(), 8080);
        assert_eq!(c.ui_addr.port(), 8081);
        assert_eq!(c.stale_run_timeout, Duration::from_secs(3600));
        assert_eq!(c.stale_run_check_interval, Duration::from_secs(60));
        assert!(c.mcp_allowed_hosts.is_empty());
    }

    #[test]
    fn mcp_allowed_hosts_are_a_comma_list() {
        let c = Config::from_lookup(lookup(&[
            ("DATABASE_URL", "x"),
            ("TR_ATTACHMENT_ROOT", "/data"),
            (
                "TR_MCP_ALLOWED_HOSTS",
                " track-record.example.com, localhost:8080 ,",
            ),
        ]))
        .unwrap();
        assert_eq!(
            c.mcp_allowed_hosts,
            ["track-record.example.com", "localhost:8080"]
        );
    }

    #[test]
    fn rejects_bad_values() {
        let base = [("DATABASE_URL", "x"), ("TR_ATTACHMENT_ROOT", "/data")];
        assert!(Config::from_lookup(lookup(&[("TR_ATTACHMENT_ROOT", "/data")])).is_err());
        let mut same = base.to_vec();
        same.extend([
            ("TR_MACHINE_ADDR", "0.0.0.0:1"),
            ("TR_UI_ADDR", "0.0.0.0:1"),
        ]);
        assert!(Config::from_lookup(lookup(&same)).is_err());
        let mut bad = base.to_vec();
        bad.push(("TR_STALE_RUN_TIMEOUT", "soon"));
        assert!(Config::from_lookup(lookup(&bad)).is_err());
        let mut zero = base.to_vec();
        zero.push(("TR_STALE_RUN_TIMEOUT", "0s"));
        assert!(Config::from_lookup(lookup(&zero)).is_err());
    }
}
