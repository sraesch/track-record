# ADR 0002: Series storage in PostgreSQL

**Status:** accepted · **Date:** 2026-11-21

## Context

A series holds up to ~10k (x, y) points. The requirements estimate up to ~10⁸
points per run (NFR-3), but we expect far less. Compressed blocks would add a
second store and custom read paths before we know we need them, so we start
simple and keep the option to switch.

## Decision

- **One row per point** in PostgreSQL: `(series_id, position, x, y)`, primary
  key `(series_id, position)`, `x`/`y` as `real` (32-bit float, ~7
  significant digits), which is enough for benchmark values.
- **Series metadata in its own table.** Points reference it by an internal
  `bigint`, not the client UUID. Hard delete cascades (FR-10).
- **One transaction per series** with bulk insert (`COPY` or multi-row
  `INSERT`). A retry with the same client ID is a no-op (FR-3).
- **Only the series store module touches points.** Derived metric reductions
  and transforms run in Rust on what it returns; any SQL pushdown stays inside
  the module.

## Consequences

- One store, one transaction, one backup; pgTAP tests series like any table.
- Switching storage changes one module; immutable data can be migrated in the
  background.
- Stored values are `f32` in Rust; reductions and transforms compute in `f64`.
  Integers are exact only up to 2²⁴ (~16.7M), so large counters such as
  transferred bytes are rounded to ~7 digits.
- ~80 bytes per point (row and index overhead dominate; `double precision`
  would be ~90). At the full estimate that is ~8 GB per run (3–15 TB per year);
  at a tenth of it, 0.3–1.5 TB per year.

## When to revisit

Track points per run, table and index size, and NFR-6 timings from day one.
Revisit when ingesting a case execution takes > 10 s, a one-year trend > 3 s,
a run comparison > 30 s, or backup/restore/vacuum become impractical.

Migration path, one step at a time:

1. One row per series with `x`/`y` as `real[]` (compressed by
   TOAST, no per-point overhead).
2. Compressed blocks in object storage (e.g. Parquet, one row group per
   series), metadata in PostgreSQL.
