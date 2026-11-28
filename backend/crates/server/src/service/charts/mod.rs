//! Charts (FR-22 to FR-27, ADR 0022). The nodes of a graph are pure code in
//! `track_record_core::chart::eval`; this module does the I/O around them.
//!
//! - [`source`] loads the frames of a source node.
//! - [`env`] gives validation the units of the metrics a spec reads.
//! - [`validate`] checks a spec, [`run`] runs one (NDJSON events or a file,
//!   [`export`]).
//! - [`model`] holds the requests and responses of the endpoints.
//!
//! A run reads all its sources in one read-only `REPEATABLE READ`
//! transaction ([`snapshot`]): the samples a source selects, their
//! attributes, the point counts it checks and the series it loads all come
//! from the same data, whatever is uploaded or invalidated meanwhile.

pub mod env;
pub mod export;
pub mod model;
pub mod run;
pub mod source;
pub mod validate;

use sqlx::{PgPool, Postgres, Transaction};

use crate::error::ApiResult;

/// A read-only transaction on one snapshot of the database.
pub async fn snapshot(pool: &PgPool) -> ApiResult<Transaction<'static, Postgres>> {
    Ok(pool
        .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await?)
}
