# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

track-record is a benchmark results store: it ingests results from an external
harness, keeps them forever, and supports human analysis (comparisons, trends,
derived metrics). [docs/requirements.md](docs/requirements.md) is the source of
truth for scope. Cite requirement IDs (`FR-n`, `NFR-n`) in commits, PRs and
design notes when a change implements or affects one.

**Status:** database schema, tests and database image exist
([db/](db/README.md)). The Rust backend ([backend/](backend/README.md)) has
ingestion, retrieval, tags/annotations, aliases, tokens, the UI-only
operations, the derived metric DSL and analysis (aggregates, trends,
comparisons, series views, export) and the MCP server. The frontend
([frontend/](frontend/README.md)) is built on the Graphite design system and
has the shell (title bar, side menu, routes for every view, color modes), the run list and the run detail (tags,
annotations, case executions, attachments with previews, invalidation,
deletion) and the case execution page (measurements per sample, a chart
per series metric with a line per sample, device details, test case tags and annotations, derived metric values,
attachments), the suites & scenarios page (versions of a suite or scenario
with what each changed, the test cases of a scenario version with their tags
and annotations), the aliases page (add, change, remove and set again device
and parameter aliases), the metrics page (one definition per metric with edit,
rename and delete, unit catalogue, custom units), the derived metrics page (list,
uses and users, editor that creates or changes a metric with live validation
and a preview on a case execution) and the API tokens page (list, create
with the secret shown once, revoke) and the About page (the app changelog,
`frontend/APP_CHANGELOG.md`, as Markdown); the other views are placeholders. Add build, test and run commands here as they appear. Do not invent commands that don't exist in the repo.

## Commands

```bash
db/test.sh                        # pgTAP tests in a throwaway PostgreSQL 18 container (needs Docker)
db/test.sh /db/tests/04_runs.sql  # selected test files (container paths)
db/image-test.sh                  # build the database image (track-record-db) and check it
docker build --tag track-record-db db   # build the database image only
db/migrate.sh                     # apply pending migrations to $PGHOST/$PGDATABASE (needs psql)
.github/scripts/cleanup-images.sh sraesch track-record-db --dry-run   # show which GHCR images cleanup would delete
backend/test.sh                   # backend: fmt, clippy, cargo test against a throwaway database (needs Docker)
backend/test.sh -- ingest         # selected backend tests (arguments go to cargo test)
cargo run -p track-record-server -- serve          # run the server (in backend/; env vars in backend/README.md)
cargo run -p track-record-server -- openapi machine > openapi/machine.json   # regenerate (in backend/; also `ui`)
docker build --file backend/Dockerfile --tag track-record-server .   # backend image (from the repo root)
docker buildx build --platform linux/amd64,linux/arm64 --file backend/Dockerfile .   # both platforms (cross-compiled)
frontend/test.sh                  # frontend: npm ci, oxlint, tsc, vitest, build (needs Node.js 24)
frontend/test.sh -- App           # selected frontend tests (arguments go to vitest run)
npm run dev                       # frontend dev server (in frontend/; proxies /ui/api to localhost:8081)
docker build --tag track-record-frontend frontend   # web UI image (nginx; proxies /ui/api to server:8081)
deploy/local/up.sh                # whole stack on Docker Desktop Kubernetes (build, push, deploy)
deploy/local/forward.sh           # UI on localhost:8088, machine API on localhost:8089
deploy/local/down.sh              # remove the local stack and its data
npm run seed                      # test data for UI development (in frontend/; needs an empty database, forward.sh)
npm run generate:api              # regenerate src/api/schema.d.ts from backend/openapi/ui.json (in frontend/)
```

## Backend

- Layout and conventions are in [ADR 0008](docs/adr/0008-backend-service.md).
  Handlers only translate HTTP; logic lives in `service/`, which the MCP
  tools (`src/mcp/`) share.
- Two ports: the machine port (`/api`, `/mcp`, API tokens) and the UI port
  (`/ui/api`, no tokens). UI-only operations (invalidate, delete, tokens,
  aliases, changing, renaming and deleting metrics, saving, renaming and
  deleting derived metrics) are routed only on the UI port.
- Results are written with `PUT` on client UUIDs: 201 on create, 200 for an
  identical retry, 409 for different content. One transaction per request.
- After changing an endpoint, regenerate `backend/openapi/*.json`; a test
  checks them. For `ui.json`, also regenerate the frontend types
  (`npm run generate:api` in `frontend/`).
- MCP (`/mcp`, rmcp, stateless) shows tools by token scope: read tools for
  every scope, tag/annotation tools only for `annotate`. Tool parameters are
  the REST request types (`schemars::JsonSchema` next to `ToSchema`); service
  errors become tool errors with the REST `code`. Tests in `tests/mcp.rs`.
- Integration tests use `tests/common` (`app!()`): a fresh database per test
  and both ports in-process. Every new endpoint gets integration tests.
- The derived metric DSL and the analysis conventions are in
  [ADR 0009](docs/adr/0009-derived-metric-dsl-and-analysis.md). The DSL
  (parse, unit check, evaluation) lives in `track-record-core::dsl` without
  I/O; `service::derived` loads definitions and sample inputs. The metric of
  every analysis view is a DSL expression.
- Derived metrics have no versions
  ([ADR 0017](docs/adr/0017-derived-metrics-without-versions.md)): `PUT`
  creates one or changes its definition, and `derived.<name>` is resolved by
  name when it is evaluated. Saving also checks the derived metrics built on
  the saved one (`broken_users` in `service::derived`).
- Uploaded metrics have no versions either
  ([ADR 0018](docs/adr/0018-metrics-without-versions.md), `service::metrics`).
  Uploads create metrics and never change them: `ensure_metric()` ignores
  the direction and description sent for an existing metric and rejects
  another unit or value type (409, `metric_definition_conflict`). The web UI
  changes, renames and deletes them. Which derived metrics use a metric is
  read from their expressions (`metric_users` in `service::derived`); a
  rename rewrites them with `dsl::rename_metric`.

## Frontend

- Layout and tooling are in [ADR 0010](docs/adr/0010-frontend-scaffold.md):
  one Vite app in `frontend/`, npm with a checked-in lockfile, oxlint.
- All user-facing text goes through `useTranslation()`; add keys to
  `src/i18n/locales/en/*.json`. Keys are typed, so unknown keys fail `tsc`.
  The no-literal-string rule is not linted; check it in review.
- Tests sit next to the code as `*.test.ts(x)` (Vitest, jsdom, Testing
  Library). Vitest globals are off: import `describe`/`it`/`expect`.
- The UI only calls the UI port (`/ui/api`), with relative URLs.
- Components are the **Graphite** design system
  ([ADR 0019](docs/adr/0019-graphite-design-system.md)), vendored in
  `frontend/graphite/` and imported as `@graphite/react`; keep local
  changes to it few and listed in `graphite/README.md`. What the app adds
  (dialogs, alerts, comboboxes, tooltips, buttons with icons, chips,
  tables, ...) is in `src/ui/` on Graphite's tokens and `g-` classes; use
  those, not Graphite's inline `Select`/`Menu`/`Tooltip`, where a table or
  dialog could clip the popup. Styling: Graphite's type classes and tokens,
  `ui-` helper classes (`src/ui/ui.css`) and a plain CSS file per area with
  `tr-<area>-` classes; no CSS-in-JS. Icons are `lucide-react`, imported by
  name. React Router ([ADR 0011](docs/adr/0011-ui-shell.md)): views are
  listed once in `src/navigation.ts` (side menu and routes); pages start
  with `<PageHeader>`. View state lives in the URL (FR-21).
- API access ([ADR 0012](docs/adr/0012-frontend-data-access.md)): types
  generated from `backend/openapi/ui.json` into `src/api/schema.d.ts`
  (`npm run generate:api`; `test.sh` fails when stale), `openapi-fetch`
  client in `src/api/client.ts`, TanStack Query for fetching. Tests fake
  the API with `mockApi()` (`src/test/api.ts`) and render with `renderApp()`.
  Put longer text into fields with `enterText()` (`src/test/input.ts`), not
  `user.type`: every keystroke renders the page again, and long texts ran
  into the timeout on loaded machines. Keep tests small; don't raise the
  timeouts further (15 s per test, 3 s for `findBy`; reasons in
  `vite.config.ts`).
- After changing a UI endpoint: regenerate the backend OpenAPI documents,
  then `npm run generate:api` in `frontend/`.
- Attachments are previewed by the app ([ADR 0013](docs/adr/0013-attachment-previews.md)),
  in a dialog or on their own page (`/runs/:runId/attachments/:id`):
  images in `<img>`, Markdown without its HTML, JSON and text as React
  text. Never render uploaded HTML, and keep the download for every file.
- Charts are Apache ECharts ([ADR 0015](docs/adr/0015-charting-library.md)),
  tree-shaken in `src/charts/echarts.ts` and loaded lazily through
  `<Chart>` (`src/charts/Chart.tsx`, own chunk). Build options in pure,
  unit-tested functions with colors from `useChartColors()` (Graphite's
  chart tokens for the resolved theme), every chart gets a translated `label` for screen readers.
  Series points come downsampled from the server (`max_points`) through
  TanStack Query (`useSeriesPoints`). Tests replace the canvas chart with
  `src/test/fakeChart.tsx`, which lists the lines.
- DSL expressions are edited with CodeMirror 6
  ([ADR 0016](docs/adr/0016-expression-editor.md)) through
  `<ExpressionField>` (`src/dsl/`, lazily loaded `editor` chunk).
  Highlighting and completion use the tokenizer in `src/dsl/syntax.ts`;
  keep its function list in step with `track-record-core::dsl`. The server
  validates while typing; its byte offsets go through `toProblems()`.
  Tests replace the editor with a text area (`src/test/fakeDslEditor.tsx`).
- The image (`frontend/Dockerfile`) is nginx serving the build and proxying
  `/ui/api` to `TR_UI_API_UPSTREAM`. `deploy/local/` runs the whole stack on
  Docker Desktop Kubernetes for trying the UI; it is not the Helm chart.

## Database

- Schema changes are new numbered files in `db/migrations/`; never edit a
  migration that has been merged. Conventions are in
  [ADR 0004](docs/adr/0004-database-schema.md).
- Every schema change comes with pgTAP tests in `db/tests/`.
- Migrations are applied only by `db/migrate.sh`, which records them with a
  checksum in `schema_migration` and refuses changed or missing ones. The
  image `track-record-db` ([ADR 0006](docs/adr/0006-database-image.md))
  applies them on first start and runs them as a job (`track-record-migrate`)
  for upgrades.
- Results are protected by triggers (`TR001`/`TR002`); never work around them.
  Hard delete only through `delete_run()`.
- Create definitions and dimensions through the `ensure_*` functions, not raw
  inserts. Metrics are created through `ensure_metric()` (uploads) and
  changed, renamed and deleted only through `save_metric()`,
  `rename_metric()` and `delete_metric()` (web UI). Derived metrics are created and changed only through
  `save_derived_metric()`, which keeps them free of cycles, and renamed and
  deleted only through `rename_derived_metric()` / `delete_derived_metric()`.

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

CI is GitHub Actions ([ADR 0005](docs/adr/0005-ci.md)): `.github/workflows/ci.yml`
runs on pull requests and on pushes to `develop` and `main`. Its jobs call the
local test commands; when a new part gets tests, add a job that runs the same
command (`backend` runs `backend/test.sh` and builds the server image,
`frontend` runs `frontend/test.sh` and builds the UI image). After
the checks pass, pushes to `develop`, `main` and `v*` tags
publish `ghcr.io/<owner>/track-record-db`, `-server` and `-frontend` for
amd64 and arm64 ([ADR 0007](docs/adr/0007-image-publishing.md),
[ADR 0014](docs/adr/0014-server-and-frontend-images.md)), each after its own
checks, through the reusable `.github/workflows/publish-image.yml`; pull
requests never publish. After each publish, `.github/scripts/cleanup-images.sh`
deletes `sha-` images beyond the newest 10 (index and children together).
Dockerfile build stages run on the build platform and cross-compile; never
build under emulation. Actions are pinned to commit hashes; keep it that way.

Backend crates are decided in ADR 0008: axum, sqlx (runtime queries),
utoipa (OpenAPI from code), rmcp for MCP; workspace in `backend/`.

Frontend tooling is decided in ADR 0010: npm, oxlint, Vitest with jsdom and
Testing Library. Routing (React Router) in ADR 0011; the Graphite design
system, Lucide icons and bundled Geist fonts in ADR 0019 (replacing MUI); API types, client and TanStack Query in ADR 0012; Markdown
rendering (react-markdown) for attachment previews in ADR 0013; charting
library (Apache ECharts) in ADR 0015; expression editor (CodeMirror 6) in
ADR 0016.

Still open: cache strategy, Helm chart, release notes.
Record design decisions as ADRs in `docs/adr/` (numbered `NNNN-title.md`), not
only in code.

## Invariants (do not break these)

**Data**
- Uploaded results are **immutable**. Corrections create new revisions; never
  update result rows in place (FR-6).
- Nothing is deleted automatically: no TTLs, retention jobs or compaction that
  drops raw samples, series or attachments (FR-8). The only deletion path is the
  explicit hard delete of a run in the web UI (FR-10).
- Suites, scenarios and aliases are **versioned**. Results reference the
  exact definition versions used (FR-7).
- Metrics are **not versioned** (FR-7, ADR 0018): one definition, which the
  web UI can change. Results reference the metric, so a change applies to
  all of them. **Never rewrite stored values** when a unit changes: they are
  read in the new unit. Uploads never change a metric; one that sends
  another unit or value type is rejected. A metric with results, or one that
  derived metrics use, cannot be deleted. Renaming one rewrites the
  expressions of the derived metrics that use it.
- Derived metrics are **not versioned**: one definition, which can be changed
  (FR-17, ADR 0017). Renaming one rewrites the expressions of the derived
  metrics that use it; one that others use cannot be deleted.
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
  a unit. A metric has one unit for all its results. Units come from the predefined catalogue; others are allowed but are
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
- Derived metrics may use other derived metrics, without cycles. A change to
  a derived metric takes effect in the metrics built on it; a change that
  would make one of them invalid is rejected on save (FR-17).
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
  the web UI. The same holds for changing, renaming and deleting metrics and
  derived metrics. Hard delete requires typing the run ID to confirm.
- MCP is read + tags/annotations only: no uploads, invalidation or deletion.

## Out of scope

Don't add benchmark execution, automatic regression detection or alerting,
authentication or user management, multi-project support, or general-purpose
time-series features.
