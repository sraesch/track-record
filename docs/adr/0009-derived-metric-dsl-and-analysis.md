# ADR 0009: Derived metric DSL and analysis

**Status:** proposed · **Date:** 2026-11-21 · the analysis views are to be
replaced by charts ([ADR 0022](0022-chart-and-dashboard-designer.md)); the
DSL stays

## Context

FR-17 and FR-17a ask for derived metrics written in a small, safe DSL that is
validated on save, is the same in the UI, REST API and MCP, and is evaluated at
query time so it also applies to historical runs. Derived metrics can use other
derived metrics without cycles. A derived metric can have no value, which is
not missing data (FR-12a). Every metric has a unit, and units in the catalogue
convert (FR-13a).

Derived metrics were versioned when this ADR was written, each version pinning
the versions it used. [ADR 0017](0017-derived-metrics-without-versions.md)
removed the versions; the section on composition below describes the current
state.

The analysis endpoints (FR-12, FR-16, FR-18 to FR-21) need one way to name the
metric of a view, and long operations must stream (NFR-7,
[ADR 0008](0008-backend-service.md)).

## Decision

### Grammar

```text
expr     = additive
additive = product (("+" | "-") product)*
product  = unary (("*" | "/") unary)*
unary    = "-" unary | power
power    = primary ("^" unary)?            right-associative; -2^2 = -(2^2)
primary  = NUMBER ("[" UNIT "]")? | name | "series." name | "derived." name
         | IDENT "(" args? ")" | "(" expr ")"
args     = arg ("," arg)*                  positional arguments before named ones
arg      = IDENT "=" expr | expr
name     = IDENT | "`" any text without "`" "`"
```

- A bare name is an **uploaded scalar metric** of the same sample.
- `series.<name>` is an **uploaded series** of the same sample.
- `derived.<name>` is a **derived metric**. The prefix is required, so a raw
  metric uploaded later under the same name cannot change what an expression
  means.
- Names with other characters are quoted: `` `load-time (cold)` ``.
- A number may have a unit: `500[ms]`. Without one it takes the unit its
  context needs (see Units).

### Functions

| Function | Arguments | Result |
|---|---|---|
| `abs`, `log10`, `sqrt` | scalar or series | same kind |
| `min`, `max` with 2+ arguments, `clamp(v, lo, hi)` | scalars or series | point by point |
| `min(s)`, `max(s)`, `mean(s)` | series, `from=`, `to=` | scalar over the points in [from, to] |
| `auc(s)` | series, `from=`, `to=`, `interp=` | area under the curve |
| `time_to(s, threshold)` | series, `from=`, `interp=` | first x ≥ from where y ≥ threshold |
| `rate(s)`, `cumsum(s)`, `log10_x(s)` | series | series |
| `moving_avg(s, window)` | series, window in x units | trailing mean over (x − window, x] |
| `rekey(s, by)` | two series, `interp=` | (by(x), y) for each point of s |

- `from=`/`to=` bounds are constants or scalars of the same sample.
- `interp=linear` (default) draws lines between points; there is no value
  outside the series. `interp=step` holds a value until the next point and
  beyond the last one.
- A repeated x is a jump; the value at that x is the last point's.
- `time_to` returns the x position (e.g. seconds since the start of the
  series), not the distance from `from`. Write `time_to(s, 0.9, from=t0) - t0`
  for the duration.
- Arithmetic between two series needs the same x values (after unit
  conversion); it is a data error otherwise. `rekey` fails if the new x would
  decrease.

### Units

The checker infers the unit of every node with the dimension algebra of the
unit catalogue (`track-record-core::units`):

- `+`, `-`, the point-by-point `min`/`max` and `clamp` need convertible
  units. The other operands are converted to the first operand with a unit.
- `*` and `/` combine units. `^` needs a constant integer exponent unless the
  base is dimensionless.
- `rate` gives y/x, `auc` y·x, `time_to` the unit of x. `log10` gives a
  dimensionless value computed in the operand's unit.
- A number without a unit adapts: in `load_time + 1` it is in the unit of
  `load_time`, in `bytes / 1000` it is a factor.
- A saved derived metric declares its unit. The inferred unit must convert to
  it, and every value is converted to it.

Units are checked twice. **On save**, against the uploaded metrics and the
most recent series of each referenced metric (a series reference needs at
least one uploaded series to know its x unit). **On evaluation**, again with
the units of each sample's inputs (cached per combination of units): the x
unit of a series can differ from one upload to the next. Inputs whose units
no longer fit are a data error for that sample. A metric has one unit
([ADR 0018](0018-metrics-without-versions.md), which replaced the metric
versions this ADR was written with); a change to it that would make a derived
metric invalid is rejected.

### Values

- Values are `Option<f64>`, computed in f64 from the f32 points. **No value**
  propagates through every operation. A non-finite result (division by zero,
  `log10` of a negative number) is no value. A threshold never reached is no
  value.
- A sample that lacks an input of the expression is **missing**: it is left
  out of the analysis and counted in `excluded.missing`, not as no value.
- Points of a series with a non-finite y count as missing points: reductions
  skip them and they are dropped from series results.

### Composition

- `derived.<name>` means the definition that derived metric has when the
  expression is evaluated. A derived metric has one definition and no
  versions ([ADR 0017](0017-derived-metrics-without-versions.md)), so a
  change to it takes effect in the metrics built on it (FR-17).
- Saving passes the names an expression refers to as `p_uses` to
  `save_derived_metric()`, which records them.
- The backend rejects a save whose dependencies include the metric itself,
  before the database's own TR007 check, and a change that would make a
  derived metric built on the saved one invalid.
- Evaluation memoizes each derived metric per sample and evaluates
  dependencies before the metric that uses them. Nesting is limited to 32
  levels, and a cycle is an error, should one exist despite the checks.
  There is no cache across requests yet (NFR-5 allows one later).

### Safety

The DSL has no loops, variables, I/O or host-language evaluation. Expressions
are limited to 4096 bytes, 64 levels of nesting and 256 levels of operations.
Series never grow during evaluation, so memory is bounded by the inputs.

### Analysis

- **The metric of every view is a DSL expression**: an uploaded metric
  (`load_time`), a derived metric (`derived.throughput`) or any expression.
  An optional `unit` converts results (FR-13). Without it, results are in the
  unit the expression has with the latest units.
- **View specs are JSON bodies of POST requests** (aggregates, trend,
  compare, series overlay and aggregate, export). They contain the run filter
  of the run list (FR-11, plus `run_id`), so the UI can keep a spec in its URL
  (FR-21). They are read-only and available with every token scope.
- **Grouping** is by run × case × client device. The case key is the scenario
  version, the asset ids and the parameters after aliases with their units.
  Devices are grouped by their name after aliases. Samples of different runs
  are never pooled.
- **Comparisons stream NDJSON** events: `start`, `progress`, one `row` per
  matching key, then `done` or `error`. Request errors are returned as
  problem details before the stream starts. **Exports stream the file**
  (CSV, NDJSON, Parquet with one row group per chunk of 2000 samples).
- Analyses touch at most 2 million samples; larger selections must be
  narrowed.
- Evaluation runs on the request's task and yields between chunks. If
  comparisons of full runs turn out to block other requests, they can move
  to a blocking thread pool without changing the API.

## Consequences

- One expression language serves saved metrics, ad-hoc analysis and (later)
  MCP, so a view spec can name anything the user can write.
- Checking units per sample costs a compile per combination of input units,
  which in practice is one per run.
- Unitless numbers are convenient but depend on the unit of their context;
  the UI should suggest `[unit]` annotations where the meaning matters.
- No cache of series reductions yet: a comparison loads every series it needs
  through the series store. NFR-6 targets are to be measured on the reference
  deployment before adding one.
