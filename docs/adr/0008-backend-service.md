# ADR 0008: Backend service

**Status:** proposed · **Date:** 2026-11-21

## Context

[ADR 0001](0001-technology-stack.md) chose Rust for all services but left the
crates, the repo layout and how long operations run open. The schema already
enforces the core invariants ([ADR 0004](0004-database-schema.md)). The
backend has to add the REST API (OpenAPI, FR-4), API token checks, the
UI-only operations (FR-9, FR-10), attachments on a shared volume
([ADR 0003](0003-attachment-storage.md)), the stale-run timeout (FR-2) and
later analysis and MCP. It must stay stateless (NFR-1).

## Decision

### Layout and crates

- **Cargo workspace in `backend/`** with two crates:
  - `track-record-core`: pure domain logic without I/O (units now; the
    statistics, series operations and derived metric DSL follow). It is easy
    to unit test and could later be compiled to WebAssembly for the UI.
  - `track-record-server`: one binary for the REST API, the MCP server (later)
    and jobs.
- **axum** (on tokio) for HTTP. **sqlx** with runtime-checked queries, so the
  build needs no database. **utoipa** with utoipa-axum generates the OpenAPI
  documents from the handlers. **rmcp** for MCP once it is added.
- **Service layer** (`service/`) between HTTP and the database. Handlers only
  translate HTTP. MCP will call the same functions, so validation, alias
  resolution and defaults are the same in every interface.
- **Series store and attachment store** are the only modules that touch
  `series_point` and attachment files (ADR 0002, ADR 0003). A test scans the
  sources and fails otherwise.

### Two listeners

One process serves two ports:

| Port | Paths | Auth | Contents |
|---|---|---|---|
| machine (8080) | `/api/v1`, `/mcp` | API token | read (any scope), tags/annotations (`annotate`), ingestion (`write`) |
| UI (8081) | `/ui/api/v1` | none (auth proxy) | read, tags/annotations, invalidate, hard delete, tokens, aliases |

UI-only operations are not routed on the machine port at all. No token
can reach them, even if the machine ingress is misconfigured (NFR-2). The
UI port has no ingestion. Each port serves its own OpenAPI document
(`/api/openapi.json`, `/ui/api/openapi.json`). Both are committed under
`backend/openapi/` for frontend type generation, and a test checks that they
are current.

Scopes: `read`; `annotate` = read + tags and annotations; `write` = read +
ingestion. Tokens are `trk_` + 32 random bytes (base64url). Only their
SHA-256 is stored, and `last_used_at` is updated at most once a minute.

### API conventions

- **Idempotent writes with PUT on client UUIDs** (FR-3). The first write
  returns 201. The same content again returns 200 with `created: false`.
  Other content under the same id returns 409 (FR-6). Each request is one
  transaction. A rejected request leaves nothing behind, not even new
  definitions.
- **Metric definitions travel with the data.** Every upload that references
  metrics carries a `metrics` map (name → unit, value type, direction,
  description). `ensure_metric_version` maps identical definitions to the
  same version.
- **Errors are RFC 9457 problem details** with a stable `code`. The
  SQLSTATEs map as follows:
  - TR001–TR004 → 409
  - TR005–TR007 → 422
  - 23505 → 409
  - 23503 and 23514 → 422
- **Validation** uses `deny_unknown_fields` plus explicit checks: finite
  values, flat scalar parameters, units only on numeric parameters,
  non-decreasing x, and values that match the metric's value type.
- **Attachments:**
  - The raw body is streamed to a temp file on the volume and hashed, with an
    optional check against `X-Content-SHA256`.
  - Under an advisory lock on the attachment id, the file is hard-linked into
    place, then the row is written.
  - Downloads always send `Content-Disposition: attachment`, `nosniff` and
    `CSP: sandbox`.

### Operation

- **Migrations stay with `db/migrate.sh`** ([ADR 0006](0006-database-image.md)).
  The build embeds the list of migrations, and the server refuses to start if
  the database lacks one.
- **Configuration** comes from environment variables: `DATABASE_URL`,
  `TR_MACHINE_ADDR`, `TR_UI_ADDR`, `TR_ATTACHMENT_ROOT`,
  `TR_STALE_RUN_TIMEOUT`, `TR_STALE_RUN_CHECK_INTERVAL`, `TR_MAX_BODY_BYTES`
  and `TR_DATABASE_MAX_CONNECTIONS`.
- **Stale-run job:** every replica runs `abort_stale_runs(timeout)` at the
  check interval inside `pg_try_advisory_xact_lock`. At most one replica does
  the work at a time, and no replica holds state.
- **Long operations** (comparisons, exports; NFR-7) will **stream** their
  results (NDJSON with progress events, streamed files) instead of running
  as stored jobs. That keeps the backend stateless and needs no job table.
- **Image:** `backend/Dockerfile` builds from the repository root (the build
  needs `db/migrations`). It exposes 8080 and 8081 and runs as a non-root
  user. CI builds it; publishing is left for later.

### Tests

`backend/test.sh` starts the database image on a random port and runs
`cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`. Each
integration test creates its own database from the migrated `track_record`
database as a template and serves both ports in-process on ephemeral ports.
Tests therefore run in parallel against the real schema, triggers included.
Without a database, plain `cargo test` runs the unit tests and skips the
database tests with a notice. The script sets `TR_REQUIRE_DB=1`, which makes
a missing database a failure instead.

## Consequences

- One binary to deploy. The Helm chart needs two Services/ingress paths: the
  machine port for `/api` and `/mcp`, and the UI port behind the auth proxy.
- Runtime-checked queries are not verified at compile time. The integration
  tests against the real schema cover them.
- Streaming instead of jobs means a dropped connection loses a long
  comparison, and the client starts it again.
- The OpenAPI documents are generated, so they cannot drift from the code.
  Changing an endpoint means regenerating the committed files.

## Left for later

The MCP tool set, image publishing for the server, and the Helm chart. The
derived metric DSL and the analysis endpoints are in
[ADR 0009](0009-derived-metric-dsl-and-analysis.md).
