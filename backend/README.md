# Backend

Rust service that ingests benchmark results and serves them over a REST
API and an MCP server. Design decisions are in
[ADR 0008](../docs/adr/0008-backend-service.md).

## Layout

```
backend/
  Cargo.toml            Workspace
  crates/core/          track-record-core: pure domain logic: units, statistics,
                        series operations, derived metric DSL (ADR 0009)
  crates/server/        track-record-server: HTTP, database, attachments, jobs
    src/api/            Handlers and the two routers
    src/service/        Service layer shared by all interfaces
    src/mcp/            MCP server and tools (/mcp on the machine port)
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
| `TR_MCP_ALLOWED_HOSTS` | empty | Comma-separated `Host` values accepted on `/mcp` (`host` or `host:port`); empty disables the check |
| `RUST_LOG` | `info` | Log filter; logs are JSON |

The server does not migrate the database. It refuses to start if a migration
is missing; apply migrations with `db/migrate.sh` or the `track-record-migrate`
job first.

The image:

```bash
docker build --file backend/Dockerfile --tag track-record-server .
```

For `linux/amd64` and `linux/arm64`, as CI publishes it
(`ghcr.io/<owner>/track-record-server`, see
[ADR 0014](../docs/adr/0014-server-and-frontend-images.md)), the build stage
cross-compiles on the build machine's platform:

```bash
docker buildx build --platform linux/amd64,linux/arm64 --file backend/Dockerfile .
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

Invalidation, hard delete, token management, alias editing and saving
derived metrics exist only on the UI port.

### Derived metrics and analysis

Derived metrics are DSL expressions over the metrics of one sample
([ADR 0009](../docs/adr/0009-derived-metric-dsl-and-analysis.md)), e.g.
`bytes / time`, `max(series.fps)`, `time_to(series.loaded, 90, from=t0)` or
`derived.throughput * 8`:

- `POST /derived-metrics/validate` checks syntax, references and units and
  returns diagnostics with byte offsets. With the `name` of an existing
  derived metric it also checks the derived metrics built on it.
- `PUT /ui/api/v1/derived-metrics/{name}` creates a derived metric (201) or
  changes its definition (200), UI port only. There are no versions
  ([ADR 0017](../docs/adr/0017-derived-metrics-without-versions.md)): a
  change applies to every analysis and to the derived metrics built on the
  changed one, and is rejected with 422 if it would make one of them invalid.
- `POST /derived-metrics/evaluate` evaluates an expression or a saved derived
  metric on the samples of one case execution.

The analysis endpoints take a JSON view spec with a run filter, the metric as
a DSL expression, an optional unit and `include_warmup`:

| Endpoint | Result |
|---|---|
| `POST /analysis/aggregates` | Statistics per run, case and device (FR-12) |
| `POST /analysis/trend` | One case, metric and device across runs, with markers and annotations (FR-18) |
| `POST /analysis/compare` | Side-by-side comparison along run, device or parameters, streamed as NDJSON (FR-16) |
| `POST /analysis/series/overlay`, `/analysis/series/aggregate` | Series on a common grid, percentile bands (FR-19) |
| `POST /analysis/export` | CSV, NDJSON or Parquet, streamed (FR-20) |

For example:

```bash
curl -s -X POST localhost:8081/ui/api/v1/analysis/aggregates \
  -H 'Content-Type: application/json' \
  -d '{"metric": "load_time", "unit": "ms", "filter": {"sut_branch": "main"}}'
```

## MCP

`/mcp` on the machine port is an MCP server (streamable HTTP, stateless, JSON
responses). It authenticates with the same API tokens as `/api`:

- any token gets the read tools: runs and case executions, the catalogue
  (suites, scenarios, test cases, metrics, devices, derived metrics,
  annotations), `aggregates`, `trend`, `compare` and the derived metric
  `validate_derived_metric` / `evaluate_derived_metric`;
- an `annotate` token also gets the tag and annotation tools.

There are no upload, invalidation or deletion tools. Tool parameters are the
REST request bodies, and results are the REST response bodies. To connect a
client, e.g. Claude Code:

```bash
claude mcp add --transport http track-record http://localhost:8080/mcp   --header "Authorization: Bearer trk_..."
```
