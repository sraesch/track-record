# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

track-record is a benchmark results store: it ingests results from an external
harness, keeps them forever, and supports human analysis (comparisons, trends,
derived metrics). [docs/requirements.md](docs/requirements.md) is the source of
truth for scope. Cite requirement IDs (`FR-n`, `NFR-n`) in commits, PRs and
design notes when a change implements or affects one.

**Status:** database schema and tests exist ([db/](db/README.md)); no backend
or frontend yet. Add build, test and run commands here as they appear. Do not
invent commands that don't exist in the repo.

## Commands

```bash
db/test.sh                        # pgTAP tests in a throwaway PostgreSQL 18 container (needs Docker)
db/test.sh /db/tests/04_runs.sql  # selected test files (container paths)
```

## Database

- Schema changes are new numbered files in `db/migrations/`; never edit a
  migration that has been merged. Conventions are in
  [ADR 0004](docs/adr/0004-database-schema.md).
- Every schema change comes with pgTAP tests in `db/tests/`.
- Results are protected by triggers (`TR001`/`TR002`); never work around them.
  Hard delete only through `delete_run()`.
- Create definitions and dimensions through the `ensure_*` functions, not raw
  inserts.

## Tech stack

Decided in [ADR 0001](docs/adr/0001-technology-stack.md):

- **Database:** PostgreSQL; database tests with **pgTAP**.
- **Backend:** all services in **Rust** (REST API, MCP server, jobs); tests with
  `cargo test`.
- **Frontend:** **TypeScript (strict)**, **React**, **Vite**; tests with
  **Vitest**; i18n with **react-i18next**. No hard-coded user-facing strings in
  components.
- **Series storage:** PostgreSQL, one row per point
  ([ADR 0002](docs/adr/0002-series-storage.md)).
- **Attachments:** files on a shared `ReadWriteMany` volume, metadata in
  PostgreSQL ([ADR 0003](docs/adr/0003-attachment-storage.md)).

Also fixed by the requirements: Kubernetes + Helm, stateless backend,
OpenAPI for the REST API.

Still open: Rust crates (HTTP, DB access/migrations, MCP), DSL grammar, cache
strategy, charting library, repo layout, CI.
Record design decisions as ADRs in `docs/adr/` (numbered `NNNN-title.md`), not
only in code.

## Invariants (do not break these)

**Data**
- Uploaded results are **immutable**. Corrections create new revisions; never
  update result rows in place (FR-6).
- Nothing is deleted automatically: no TTLs, retention jobs or compaction that
  drops raw samples, series or attachments (FR-8). The only deletion path is the
  explicit hard delete of a run in the web UI (FR-10).
- Suites, scenarios, metric definitions, derived metrics and aliases are
  **versioned**. Results reference the exact definition versions used (FR-7).
- A changed scenario definition is a **new scenario version and a different
  case**. Never mix old and new versions in one trend.
- Client device identity is its **stable name**. Device detail changes (OS,
  driver, …) are marked in trends but must not split them.
- Trend comparison key: **case + client device name, after aliases are applied**.
- Shared environments are deduplicated by fingerprint.
- Parameters keep their JSON type (number/string/bool). Don't stringify them.
- Only **raw, measured** values and series are uploaded. Anything computed from
  them is a derived metric (FR-17), never an uploaded measurement.
- One series per metric per sample. x never decreases but may repeat.
- Every metric, series x-dimension and, where it applies, numeric parameter has
  a unit. Units come from the predefined catalogue; others are allowed but are
  not converted (FR-13a).

**Ingestion**
- Incremental: create run → add case executions/samples/series → close run.
  Partial data must survive a harness crash (FR-1).
- Status lifecycle `running → completed | failed | aborted`; stale `running`
  runs time out to `aborted` (FR-2). Stale means no upload for the timeout in
  **server time** since the last upload (creating the run counts). Never use the
  measured start time: runs are also uploaded after the fact or imported.
- Run start/end are measured times from the harness. Trends and time filters
  use them, not the upload time.
- Every write is **idempotent** through client-generated IDs (FR-3) and
  validated against the published schema (FR-4).

**Storage and performance**
- Series points are stored in PostgreSQL, **one row per point**
  ([ADR 0002](docs/adr/0002-series-storage.md), NFR-4). This is deliberately
  simple; don't optimize it without measurements.
- All reads and writes of points go through the **series store module**. No
  other code touches the points table, so storage can later move to arrays or
  compressed blocks without changing anything else. Derived metric reductions
  and transforms run in Rust on series returned by that module.
- A series and all its points are written in one transaction (bulk insert).
- All attachment file access goes through the **attachment store module**.
  Files are written atomically (temp file + rename), never overwritten, and
  stored under their ID, never under a user-supplied name.
- Caches (e.g. series reduction results) are optional, never the source of
  truth, and must be rebuildable from raw data (NFR-5).
- Operations that may take more than a few seconds (comparisons, exports)
  stream results or run as pollable jobs, and the UI shows progress (NFR-7).
- Keep the backend stateless so it scales horizontally (NFR-1).

**Analysis**
- Invalid runs are excluded from analysis by default but can be included on
  request (FR-9). Warmup samples are excluded from aggregation by default (FR-12).
- Aliases apply equally in UI, REST API and MCP. Put alias resolution in the
  shared query layer, not in one interface. Aliases also cover case parameter
  values and keep their JSON type.
- Comparison (FR-16) runs along a dimension: the run, the client device or case
  parameters. Everything not compared is the matching key; the user may take
  parameters out of it. **Never pool samples of different runs.** Mark columns
  that come from different SUT versions or shared environments.
- A derived metric can have **no value** (e.g. threshold never reached); an
  uploaded measurement always has one. No value is not missing data: show it,
  count it in aggregates, and don't compute differences against it (FR-12a).
- Derived metrics may use other derived metrics, without cycles. Each version
  pins the exact versions it uses (FR-17).
- Every run, case execution and analysis view has a URL that reproduces it,
  filters included. Keep view state in the URL (FR-21).
- The derived metric DSL is the **same** in UI, API and MCP, and must stay
  safe: a fixed set of operators and functions, no loops, no I/O, no
  `eval` of host-language code. Validate on save: syntax, referenced metrics
  exist, unit consistency (FR-17a).
- No cross-run expressions in the DSL; comparison against a reference is done
  through normalization (FR-13).

## Access control

- No user identity in the app. Don't add users, roles or annotation authors.
- `/api` and `/mcp` authenticate with API tokens (stored only as hashes).
  Scopes: `read`, `annotate` (read + tags/annotations), `write` (ingestion,
  including the tags a run is created with).
- **Invalidation and deletion must not be reachable with any token**, only from
  the web UI. Hard delete requires typing the run ID to confirm.
- MCP is read + tags/annotations only: no uploads, invalidation or deletion.

## Out of scope

Don't add benchmark execution, automatic regression detection or alerting,
authentication or user management, multi-project support, or general-purpose
time-series features.
