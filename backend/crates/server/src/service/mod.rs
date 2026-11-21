//! Service layer shared by the REST API (both ports) and the MCP server.
//! Handlers only translate HTTP to these functions, so every interface gets
//! the same validation, alias resolution and defaults.

pub mod annotations;
pub mod attachments;
pub mod catalog;
pub mod ingest;
pub mod lifecycle;
pub mod query;
pub mod runs;
pub mod tokens;
