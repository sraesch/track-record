# ADR 0008: Backend service

**Status:** proposed · **Date:** 2026-11-21

## Context

[ADR 0001](0001-technology-stack.md) chose Rust for all services but left the
crates, the repo layout and how long operations run open. The schema already
enforces the core invariants ([ADR 0004](0004-database-schema.md)). The
backend has to add the REST API (OpenAPI, FR-4), API token checks, the
UI-only operations (FR-9, FR-10), attachments on a shared volume
([ADR 0003](0003-attachment-storage.md)), the stale-run timeout (FR-2),
analysis and the MCP server. It must stay stateless (NFR-1).

## Decision

### Layout and crates

- **Cargo workspace in `backend/`** with two crates:
  - `track-record-core`: pure domain logic without I/O: units, statistics,
    series operations and the derived metric DSL. It is easy to unit test and
    could later be compiled to WebAssembly for the UI.
  - `track-record-server`: one binary for the REST API, the MCP server and
    jobs.
- **axum** (on tokio) for HTTP. **sqlx** with runtime-checked queries, so the
  build needs no database. **utoipa** with utoipa-axum generates the OpenAPI
  documents from the handlers. **rmcp** (the official Rust MCP SDK) for MCP,
with **schemars** for the tool input schemas.
- **Service layer** (`service/`) between HTTP and the database. Handlers only
  translate HTTP, and MCP tools only translate MCP. Both call the same
  functions, so validation, alias resolution and defaults are the same in
  every interface.
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

### MCP server

`/mcp` on the machine port is the MCP server of requirements §4, over
**streamable HTTP in stateless mode with JSON responses**: no MCP sessions,
every POST is handled by a fresh server instance, nothing is kept between
requests (NFR-1). It is behind the same token middleware as `/api`, so a
missing or revoked token gets 401 before MCP is involved.

- **Tools follow the token scope.** Every scope gets the read tools; only
  `annotate` also gets the tag and annotation tools. `tools/list` shows only
  the tools the token may use, and calling another one fails as if it did not
  exist. A `write` token (ingestion) gets the read tools only.
- **Read tools:** `list_runs`, `get_run`, `list_case_executions`,
  `get_case_execution`, `list_suites`, `list_scenarios`, `list_test_cases`,
  `list_metrics`, `list_client_devices`, `list_derived_metrics`,
  `list_annotations`, `aggregates`, `trend`, `compare`,
  `validate_derived_metric`, `evaluate_derived_metric`. They are marked
  read-only.
- **Annotate tools:** `add_run_tag`, `remove_run_tag`, `add_test_case_tag`,
  `remove_test_case_tag`, `create_annotation`, `update_annotation`,
  `delete_annotation`.
- There are **no tools for uploads, invalidation, deletion, tokens, aliases
  or saving derived metrics**.
- Tool parameters are the REST request types (they derive `JsonSchema` next
  to `ToSchema`), so a view spec means the same in both. Results are the REST
  response bodies as structured content; lists are wrapped in
  `{"items": [...]}` because structured content must be an object.
- Service errors become **tool errors** (`isError`) with the REST `status`,
  `code` and `detail`, so the model sees them. Invalid arguments are tool
  errors as well.
- `compare` collects the NDJSON stream into one result (`start` fields, all
  rows, `excluded`). MCP has no streamed tool results; very large comparisons
  should use the REST endpoint.
- The `Host` check that rmcp applies against DNS rebinding is off unless
  `TR_MCP_ALLOWED_HOSTS` lists hosts: the bearer token already keeps browsers
  out, and the public hostname is only known at deployment.

### API conventions

- **Idempotent writes with PUT on client UUIDs** (FR-3). The first write
  returns 201. The same content again returns 200 with `created: false`.
  Other content under the same id returns 409 (FR-6). Each request is one
  transaction. A rejected request leaves nothing behind, not even new
  definitions.
- **Metric definitions travel with the data.** Every upload that references
  metrics carries a `metrics` map (name → unit, value type, direction,
  description). `ensure_metric` creates the metrics that do not exist yet.
  It never changes one, and rejects another unit or value type (TR009 → 409):
  metrics have one definition, which is changed in the web UI
  ([ADR 0018](0018-metrics-without-versions.md), which replaced the metric
  versions this ADR first described).
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
  `TR_STALE_RUN_TIMEOUT`, `TR_STALE_RUN_CHECK_INTERVAL`, `TR_MAX_BODY_BYTES`,
  `TR_DATABASE_MAX_CONNECTIONS` and `TR_MCP_ALLOWED_HOSTS`.
- **Stale-run job:** every replica runs `abort_stale_runs(timeout)` at the
  check interval inside `pg_try_advisory_xact_lock`. At most one replica does
  the work at a time, and no replica holds state.
- **Long operations** (comparisons, exports; NFR-7) **stream** their
  results (NDJSON with progress events, streamed files) instead of running
  as stored jobs. That keeps the backend stateless and needs no job table.
- **Image:** `backend/Dockerfile` builds from the repository root (the build
  needs `db/migrations`). It exposes 8080 and 8081 and runs as a non-root
  user. CI builds it; publishing is left for later. The runtime base is
  `gcr.io/distroless/cc-debian13:nonroot` (uid 65532), not `debian:*-slim`:
  the binary links only libc and libgcc_s (TLS is rustls with built-in root
  certificates), and a full Debian base brings packages the server never
  uses (util-linux, perl, sqlite, ncurses) that vulnerability scanners
  flag. The build stage must use the same Debian release, so the glibc
  matches.

### Tests

`backend/test.sh` starts the database image on a random port and runs
`cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`. Each
integration test creates its own database from the migrated `track_record`
database as a template and serves both ports in-process on ephemeral ports.
Tests therefore run in parallel against the real schema, triggers included.
The MCP tests use the rmcp client over HTTP against the in-process server.
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

Image publishing for the server and the Helm chart. The
derived metric DSL and the analysis endpoints are in
[ADR 0009](0009-derived-metric-dsl-and-analysis.md).
