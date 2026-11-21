# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

track-record is a benchmark results store: it ingests results from an external
harness, keeps them forever, and supports human analysis (comparisons, trends,
derived metrics). [docs/requirements.md](docs/requirements.md) is the source of
truth for scope. Cite requirement IDs (`FR-n`, `NFR-n`) in commits, PRs and
design notes when a change implements or affects one.

**Status:** no code yet. Tech stack, build, test and run commands are still to be
decided. When they are, add them to this file. Do not invent commands that don't
exist in the repo.

Decided by the requirements: Kubernetes + Helm, stateless backend, a relational
DB (e.g. PostgreSQL) for metadata, S3-compatible object storage, OpenAPI for the
REST API, an MCP server, a web UI.

Left open as design decisions: series block format, DSL grammar, cache strategy.
Record such decisions in `docs/` (e.g. `docs/adr/`) rather than only in code.

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

**Ingestion**
- Incremental: create run → add case executions/samples/series → close run.
  Partial data must survive a harness crash (FR-1).
- Status lifecycle `running → completed | failed | aborted`; stale `running`
  runs time out to `aborted` (FR-2).
- Every write is **idempotent** through client-generated IDs (FR-3) and
  validated against the published schema (FR-4).

**Storage and performance**
- Series are **never stored one row per point**. Each series is a compressed
  block (object storage or blob), with only metadata in the relational DB
  (NFR-4). Volume is ~10⁸ points per run and terabytes over the lifetime.
- Caches (e.g. series reduction results) are optional, never the source of
  truth, and must be rebuildable from raw data (NFR-5).
- Operations that may take more than a few seconds (comparisons, exports)
  stream results or run as pollable jobs, and the UI shows progress (NFR-7).
- Keep the backend stateless so it scales horizontally (NFR-1).

**Analysis**
- Invalid runs are excluded from analysis by default but can be included on
  request (FR-9). Warmup samples are excluded from aggregation by default (FR-12).
- Aliases apply equally in UI, REST API and MCP. Put alias resolution in the
  shared query layer, not in one interface.
- The derived metric DSL is the **same** in UI, API and MCP, and must stay
  safe: a fixed set of operators and functions, no loops, no I/O, no
  `eval` of host-language code. Validate on save: syntax, referenced metrics
  exist, unit consistency (FR-17a).
- No cross-run expressions in the DSL; comparison against a reference is done
  through normalization (FR-13).

## Access control

- No user identity in the app. Don't add users, roles or annotation authors.
- `/api` and `/mcp` authenticate with API tokens (stored only as hashes).
  Scopes: `read`, `annotate` (read + tags/annotations), `write` (ingestion).
- **Invalidation and deletion must not be reachable with any token**, only from
  the web UI. Hard delete requires typing the run ID to confirm.
- MCP is read + tags/annotations only: no uploads, invalidation or deletion.

## Out of scope

Don't add benchmark execution, automatic regression detection or alerting,
authentication or user management, multi-project support, or general-purpose
time-series features.
