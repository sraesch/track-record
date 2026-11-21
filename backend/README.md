# Backend

Rust service that ingests benchmark results and serves them over a REST
API. Design decisions are in [ADR 0008](../docs/adr/0008-backend-service.md).

## Layout

```
backend/
  Cargo.toml            Workspace
  crates/core/          track-record-core: pure domain logic (units)
  crates/server/        track-record-server: HTTP, database, attachments, jobs
    src/api/            Handlers and the two routers
    src/service/        Service layer shared by all interfaces
    src/series_store.rs The only code touching series points
    src/attachment_store.rs  The only code touching attachment files
    tests/              Integration tests (one database per test)
  openapi/              Generated OpenAPI documents (machine.json, ui.json)
  Dockerfile            Server image (build context: repository root)
  test.sh               fmt, clippy and all tests against a throwaway database
```

## Running the tests

Needs Docker and a Rust toolchain:

```bash
backend/test.sh
```

The script starts the `track-record-db` image (built from `db/`) on a random
port, then runs `cargo fmt --check`, `cargo clippy` and `cargo test`.
Arguments after `--` go to `cargo test`, e.g. `backend/test.sh -- ingest`.
Plain `cargo test` without a database runs only the unit tests.

## Running the server

Start a database with the schema (see [db/README.md](../db/README.md)), then:

```bash
DATABASE_URL=postgres://postgres:secret@localhost:5432/track_record \
TR_ATTACHMENT_ROOT=/tmp/track-record-attachments \
cargo run -p track-record-server -- serve
```

| Variable | Default | Meaning |
|---|---|---|
| `DATABASE_URL` | required | PostgreSQL URL |
| `TR_ATTACHMENT_ROOT` | required | Root of the shared attachment volume |
| `TR_MACHINE_ADDR` | `0.0.0.0:8080` | Machine port: `/api`, `/mcp` with API tokens |
| `TR_UI_ADDR` | `0.0.0.0:8081` | UI port: `/ui/api` behind the auth proxy, no tokens |
| `TR_STALE_RUN_TIMEOUT` | `1h` | Running runs without uploads for this long are aborted (FR-2) |
| `TR_STALE_RUN_CHECK_INTERVAL` | `1m` | How often to check |
| `TR_MAX_BODY_BYTES` | 256 MiB | Maximum JSON request body |
| `TR_DATABASE_MAX_CONNECTIONS` | `16` | Pool size |
| `RUST_LOG` | `info` | Log filter; logs are JSON |

The server does not migrate the database. It refuses to start if a migration
is missing; apply migrations with `db/migrate.sh` or the `track-record-migrate`
job first.

The image:

```bash
docker build --file backend/Dockerfile --tag track-record-server .
```

## API

Each port serves its own OpenAPI document: `/api/openapi.json` (machine)
and `/ui/api/openapi.json` (UI). Both are also committed in `openapi/`. After
changing an endpoint, regenerate them (a test fails otherwise):

```bash
cargo run -p track-record-server -- openapi machine > backend/openapi/machine.json
cargo run -p track-record-server -- openapi ui > backend/openapi/ui.json
```

Ingestion on the machine port needs a `write` token, created on the UI port
(`POST /ui/api/v1/tokens`). An upload looks like this:

1. `PUT /api/v1/runs/{run_id}` creates the run. It starts as `running`.
2. `PUT /api/v1/runs/{run_id}/case-executions/{id}` adds a case execution,
   optionally with its samples and series.
3. `PUT .../samples/{id}` and `PUT .../samples/{id}/series/{id}` add samples
   and series incrementally.
4. `PUT /api/v1/runs/{run_id}/attachments/{id}?file_name=...` uploads an
   attachment as the raw body.
5. `POST /api/v1/runs/{run_id}/close` sets `completed` or `failed`.

All ids are client-generated UUIDs. The first write returns 201. Repeating it
with the same content returns 200. Sending other content under the same id
returns 409. Errors are `application/problem+json` with a stable `code`.

Invalidation, hard delete, token management and alias editing exist only on
the UI port.
