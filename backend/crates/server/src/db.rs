//! Database connection, schema version check and the enum types of the
//! schema (migration 0001).

use std::collections::BTreeSet;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use utoipa::ToSchema;

/// Migrations this build expects, as (version, name), from db/migrations.
pub const EXPECTED_MIGRATIONS: &[(i32, &str)] =
    include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

pub async fn connect(url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .context("cannot connect to the database")
}

/// Fails if the database lacks a migration this build expects. Migrations
/// are applied by db/migrate.sh (ADR 0006), never by the server. A database
/// with newer migrations is accepted: they only add.
pub async fn check_schema(pool: &PgPool) -> anyhow::Result<()> {
    let applied: Vec<i32> = sqlx::query_scalar("SELECT version FROM schema_migration")
        .fetch_all(pool)
        .await
        .context("cannot read schema_migration; was db/migrate.sh run?")?;
    let applied: BTreeSet<i32> = applied.into_iter().collect();
    let missing: Vec<String> = EXPECTED_MIGRATIONS
        .iter()
        .filter(|(v, _)| !applied.contains(v))
        .map(|(v, name)| format!("{v:04}_{name}"))
        .collect();
    if !missing.is_empty() {
        bail!(
            "database schema is behind; missing migrations: {}",
            missing.join(", ")
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "run_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "run_trigger", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    Nightly,
    Manual,
    Ci,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "metric_value_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MetricValueType {
    Float,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "metric_direction", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    HigherIsBetter,
    LowerIsBetter,
}

/// API token scope (NFR-2). `annotate` includes `read`; `write` (ingestion)
/// includes `read`. Invalidation and deletion need no scope because no token
/// can reach them: they exist only on the UI port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "token_scope", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TokenScope {
    Read,
    Annotate,
    Write,
}

impl TokenScope {
    /// True if a token with this scope may use an endpoint that requires
    /// `required`.
    pub fn allows(self, required: TokenScope) -> bool {
        match required {
            TokenScope::Read => true,
            TokenScope::Annotate => self == TokenScope::Annotate,
            TokenScope::Write => self == TokenScope::Write,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_include_read_but_not_each_other() {
        use TokenScope::*;
        for scope in [Read, Annotate, Write] {
            assert!(scope.allows(Read));
        }
        assert!(Annotate.allows(Annotate));
        assert!(!Annotate.allows(Write));
        assert!(Write.allows(Write));
        assert!(!Write.allows(Annotate));
        assert!(!Read.allows(Annotate));
        assert!(!Read.allows(Write));
    }

    #[test]
    fn expected_migrations_are_embedded_in_order() {
        assert!(EXPECTED_MIGRATIONS.len() >= 10);
        assert_eq!(EXPECTED_MIGRATIONS[0], (1, "foundation"));
        assert!(EXPECTED_MIGRATIONS.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
