# ADR 0004: Database schema conventions

**Status:** proposed · **Date:** 2026-11-21

## Context

The schema has to enforce the core invariants (immutability, versioning,
idempotency, run lifecycle) no matter which backend code writes to it. The
schema itself is in [db/](../../db/README.md); this ADR records the rules
behind it.

## Decision

- **Migrations** are plain SQL files `db/migrations/NNNN_name.sql`, applied
  in order, one transaction each, by `db/migrate.sh`
  ([ADR 0006](0006-database-image.md)). This naming also works with `sqlx`.
- **IDs:** results (runs, case executions, samples, series, attachments) use
  client-generated UUIDs as primary keys, so a retried upload hits the
  existing row (FR-3). Definitions and dimensions use `bigint` identities.
- **Immutability in the database** (FR-6, FR-8): a statement-level trigger
  rejects `UPDATE`, `DELETE` and `TRUNCATE` on definitions, dimensions and
  results. On `run`, only status, end time and validity can change.
- **Hard delete** (FR-10) only through `delete_run()`, which sets a
  transaction-local flag that lets the cascade through. Definitions and
  dimensions are never deleted.
- **Deduplication and versioning in the database:** `ensure_*` functions
  return the matching row, creating it if needed, and lock the parent row so
  concurrent uploads get consistent version numbers.
  - Harness-sent definitions (suites, scenarios, metrics), environments and
    test cases are deduplicated by content hash, so sending an old definition
    returns its old version.
  - Aliases, which are edited in the UI, get a new version unless the content
    equals the latest one.
- **Derived metrics are not versioned**
  ([ADR 0017](0017-derived-metrics-without-versions.md), which replaced the
  versions this ADR first described). `derived_metric` holds the one
  definition and can be updated; its name cannot change and it is not
  deleted. `derived_metric_dependency` records which derived metrics an
  expression uses and is replaced with the expression.
  `save_derived_metric()` rejects a metric that would depend on itself
  (FR-17).
- **Alias values are JSON scalars** (`jsonb`), so aliases on case parameter
  values keep their type: `10` and `"10"` are different values.
- **Units** (FR-13a) are stored as symbols (text) on metrics, series
  x-dimensions and numeric parameters (`test_case.parameter_units`, part of
  the test case fingerprint). The `unit` table is the predefined catalogue
  with one conversion factor per unit; `unit_conversion_factor()` converts
  within a quantity. Symbols outside the catalogue are accepted and not
  converted. Units that need an offset (°C) are not in the catalogue.
- **Environments** store both roles (shared environment, client device
  details) in one table, deduplicated by fingerprint. A device detail change
  shows up as a new `device_environment_id` on the same `client_device_id`.
- **Corrections** (FR-6) are new case executions with `supersedes_id`
  pointing to the replaced one in the same run, test case and device. They
  may be added to finished runs. `current_case_execution` hides superseded
  rows.
- **Run lifecycle** (FR-1, FR-2): runs are created `running`, move once to a
  terminal status, and only accept new case executions while running.
  `abort_stale_runs(timeout)` aborts runs that received no upload (run, case
  execution, sample, series or attachment) within the timeout. It counts
  server time (`created_at`), never the measured `started_at`, so runs that
  are uploaded after the fact or imported are not aborted.
- **Raw values only:** measurements are finite numbers and there is one
  series per sample and metric. Anything computed, including a curve over
  another x-dimension, is a derived metric.
- **Series completeness** (ADR 0002): a deferred constraint trigger checks at
  commit that a new series has exactly `point_count` points and that x never
  decreases (equal x values are allowed); points added later are rejected.
- **Errors** use custom SQLSTATEs `TR001`–`TR007` (see
  [db/README.md](../../db/README.md#error-codes)).
- **Tests** are pgTAP files in `db/tests/`, run by `db/test.sh` in a
  throwaway PostgreSQL 18 container.

## Consequences

- Invariants hold even for buggy or future writers, and pgTAP can test them
  directly.
- The flag in `delete_run()` guards against mistakes, not against a client
  that sets it on purpose. Database roles and grants can add a hard barrier
  later.
- Deleting a large run cascades through every point row and may take a
  while; that is acceptable for a rare, manual action.
- Some ingestion logic (dedup, versioning) lives in SQL functions rather than
  in Rust.
