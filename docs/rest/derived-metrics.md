# Derived metrics

A derived metric is a named expression over the measurements and series of
**one sample** (FR-17), e.g. throughput from bytes and load time, or the
time until a frame rate is reached. It is computed when it is read, so a new
or changed derived metric applies to all stored runs. Every
[analysis](analysis.md) view takes its metric as such an expression, saved
or ad hoc.

Derived metrics have **one** definition and no versions
([ADR 0017](../adr/0017-derived-metrics-without-versions.md)). They are
saved, renamed and deleted only in the web UI. The machine API reads them
and validates and evaluates expressions. Every endpoint on this page works
with a token of any scope.

The expression language is described [below](#expression-language).

## List derived metrics

```http
GET /api/v1/derived-metrics
```

All derived metrics, by name. Response: `DerivedMetric`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Id |
| `name` | string | Name; expressions refer to it as `derived.<name>` |
| `expression` | string | The expression |
| `unit` | string | Declared unit; every value is converted to it |
| `description` | string | Free text |
| `created_at` | timestamp | |
| `updated_at` | timestamp | When the definition was last changed |
| `uses` | string[] | Names of the derived metrics the expression uses directly |

```json
[
  {
    "id": 1,
    "name": "throughput",
    "expression": "bytes_received / load_time",
    "unit": "MB/s",
    "description": "Download throughput while loading",
    "created_at": "2026-11-23T20:03:29.603612Z",
    "updated_at": "2026-11-23T20:03:29.603612Z",
    "uses": []
  },
  {
    "id": 2,
    "name": "throughput_mbit",
    "expression": "derived.throughput * 8",
    "unit": "Mbit/s",
    "description": "",
    "created_at": "2026-11-23T20:03:29.635439Z",
    "updated_at": "2026-11-23T20:03:29.635439Z",
    "uses": ["throughput"]
  }
]
```

## Get a derived metric

```http
GET /api/v1/derived-metrics/{name}
```

Response: one `DerivedMetric`. `404 not_found` if there is none with this
name. Percent-encode names with special characters.

## Validate an expression

```http
POST /api/v1/derived-metrics/validate
```

Checks an expression against the current metrics and derived metrics
(FR-17a): syntax, that every referenced metric exists, units, and cycles.
It **always answers `200`**; `valid` says whether the expression passed and
`diagnostics` point at the problems.

Request body `ValidateExpression`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `expression` | string | yes | The expression |
| `unit` | string \| null | no | Unit the result must convert to |
| `name` | string \| null | no | Name the expression would be saved under. A reference to that name, directly or through other derived metrics, is a cycle. If a derived metric of that name exists, the derived metrics built on it are checked as well: the expression is invalid if it would break one of them |

Response `Validation`:

| Field | Type | Meaning |
|---|---|---|
| `valid` | boolean | Whether the expression passed |
| `diagnostics` | `DslDiagnostic`[] | Problems: `message`, and `start`/`end` as **byte** offsets into the expression |
| `kind` | `scalar` \| `series` \| null | What the expression yields, if valid |
| `unit` | string \| null | Unit of the result (the requested `unit` if given), if valid |
| `inferred_unit` | string \| null | Unit the expression has by itself, if valid |
| `x_unit`, `x_dimension` | string \| null | For a series result: unit and dimension of x |
| `uses` | string[] | Derived metrics the expression refers to directly |
| `inputs` | object | `metrics` and `series`: the uploaded scalar metrics and series it needs, also through the derived metrics it uses |

Series references are checked against the most recent uploaded series of
that metric, which supplies the x unit. A series that has never been
uploaded cannot be checked.

```bash
curl -s -X POST "$TR_URL/api/v1/derived-metrics/validate" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"expression": "bytes_received / load_time", "unit": "MB/s"}'
```

```json
{
  "valid": true,
  "diagnostics": [],
  "kind": "scalar",
  "unit": "MB/s",
  "inferred_unit": "B/ms",
  "x_unit": null,
  "x_dimension": null,
  "uses": [],
  "inputs": {"metrics": ["bytes_received", "load_time"], "series": []}
}
```

An invalid one:

```json
{
  "valid": false,
  "diagnostics": [
    {
      "message": "'+' needs compatible units, but this is in \"B\" and the first operand in \"ms\"",
      "start": 12,
      "end": 26
    }
  ],
  "kind": null,
  "unit": null,
  "inferred_unit": null,
  "x_unit": null,
  "x_dimension": null,
  "uses": [],
  "inputs": {"metrics": ["bytes_received", "load_time"], "series": []}
}
```

(for `load_time + bytes_received`; bytes 12–26 are `bytes_received`).

## Evaluate an expression

```http
POST /api/v1/derived-metrics/evaluate
```

Evaluates an ad-hoc expression or a saved derived metric on **every sample
of one case execution**, warmup samples included. Useful to try an
expression before saving it, or to see why a sample has no value.

Request body `EvaluateRequest`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `case_execution_id` | UUID | yes | The case execution |
| `expression` | string \| null | one of the two | An ad-hoc expression |
| `name` | string \| null | | A saved derived metric |
| `unit` | string \| null | no | Output unit. Default: the declared unit of a saved metric, or the expression's own unit |
| `max_points` | integer \| null | no | For series results: downsample each to at most this many points (at least 2) |

Response `EvaluateResponse`:

| Field | Type | Meaning |
|---|---|---|
| `expression` | string | The expression (`derived.<name>` for a saved one) |
| `kind` | `scalar` \| `series` | |
| `unit` | string | Unit of the values |
| `x_unit`, `x_dimension` | string \| null | For series results |
| `samples` | `SampleResult`[] | One per sample, by repetition |

`SampleResult`:

| Field | Type | Meaning |
|---|---|---|
| `sample_id`, `repetition`, `is_warmup` | | The sample |
| `status` | `value` \| `no_value` \| `missing` \| `error` | See below |
| `value` | number \| null | Scalar result |
| `series` | object \| null | Series result: `x` (in `x_unit`) and `y` |
| `message` | string \| null | Why the status is `missing` or `error` |

| Status | Meaning |
|---|---|
| `value` | The sample has a value |
| `no_value` | The expression has **no value** for this sample, e.g. a threshold that was never reached or a division by zero. This is a result, not missing data (FR-12a) |
| `missing` | The sample lacks an input of the expression (the metric was not measured). Analysis leaves such samples out |
| `error` | The inputs do not fit the expression, e.g. units that do not convert or series with different x values |

```bash
curl -s -X POST "$TR_URL/api/v1/derived-metrics/evaluate" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"case_execution_id": "5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11", "expression": "time_to(series.fps, 55)", "unit": "ms"}'
```

```json
{
  "expression": "time_to(series.fps, 55)",
  "kind": "scalar",
  "unit": "ms",
  "x_unit": null,
  "x_dimension": null,
  "samples": [
    {"sample_id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c10", "repetition": 0, "is_warmup": true,
     "status": "value", "value": 1000.0, "series": null, "message": null},
    {"sample_id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11", "repetition": 1, "is_warmup": false,
     "status": "value", "value": 1000.0, "series": null, "message": null}
  ]
}
```

A series result, `moving_avg(series.fps, 1[s])`:

```json
{
  "expression": "moving_avg(series.fps, 1[s])",
  "kind": "series",
  "unit": "fps",
  "x_unit": "s",
  "x_dimension": "time",
  "samples": [
    {"sample_id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11", "repetition": 1, "is_warmup": false,
     "status": "value", "value": null,
     "series": {"x": [0.0, 0.5, 1.0, 1.5, 2.0], "y": [0.0, 12.5, 40.0, 57.5, 59.75]},
     "message": null}
  ]
}
```

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | Neither or both of `expression` and `name`; `max_points` below 2 |
| `404` | `not_found` | No such case execution or derived metric |
| `422` | `invalid_expression` | The expression is invalid; `detail` lists the problems |

## Expression language

The language is the same in the web UI, the REST API and MCP
([ADR 0009](../adr/0009-derived-metric-dsl-and-analysis.md)). It is
deliberately small and safe: numbers, references, arithmetic and a fixed
set of functions. There are no variables, loops, I/O or host-language
evaluation.

### References

| Syntax | Means |
|---|---|
| `load_time` | The uploaded **scalar** metric `load_time` of the same sample |
| `series.fps` | The uploaded **series** of metric `fps` of the same sample |
| `derived.throughput` | The derived metric `throughput`, as defined when the expression is evaluated |
| `` `load-time (cold)` `` | A name that is not a plain identifier, in backticks. Also after the prefixes: `` series.`frame time` ``, `` derived.`90th` `` |

Plain identifiers start with a letter or `_` and continue with letters,
digits and `_`. The `derived.` prefix is required, so a metric uploaded
later under the same name cannot change what an expression means.

### Numbers and operators

- Numbers: `42`, `0.5`, `1e6`. A number can carry a unit: `500[ms]`,
  `1[s]`, `8[bit]`.
- Operators, by increasing precedence: `+ -`, `* /`, unary `-`, `^`.
  `^` is right-associative and binds tighter than unary minus:
  `-2^2 = -(2^2)`.
- Parentheses group.

### Functions

| Function | Arguments | Result |
|---|---|---|
| `abs(x)`, `sqrt(x)`, `log10(x)` | scalar or series | Same kind, point by point. `log10` is dimensionless |
| `min(a, b, ...)`, `max(a, b, ...)` | 2 to 16 scalars or series | Point-by-point minimum/maximum |
| `clamp(v, lo, hi)` | scalars or series | `v` limited to `[lo, hi]` |
| `min(s)`, `max(s)`, `mean(s)` | series; `from=`, `to=` | Scalar over the points with x in `[from, to]` |
| `auc(s)` | series; `from=`, `to=`, `interp=` | Area under the curve, unit y·x |
| `time_to(s, threshold)` | series, scalar; `from=`, `interp=` | First x ≥ `from` where y ≥ `threshold`; unit of x |
| `rate(s)` | series | Derivative: the slope between neighbouring points, at the later x; unit y/x |
| `cumsum(s)` | series | Running sum of y |
| `log10_x(s)` | series | The series with x replaced by log10(x) (dimensionless); points with x ≤ 0 are dropped |
| `moving_avg(s, window)` | series, window in x units | Trailing mean over `(x − window, x]` |
| `rekey(s, by)` | two series; `interp=` | Points `(by(x), y)`: `s` against the values of `by` instead of x. Fails if the new x would decrease |

Named arguments come after positional ones:
`mean(series.fps, from=1[s], to=5[s])`.

- `from=` and `to=` are constants or scalars of the same sample, e.g.
  `from=t0`.
- `interp=linear` (default) draws straight lines between points; there is
  no value outside the series. `interp=step` holds each value until the next
  point and beyond the last one.
- A repeated x is a jump; the value at that x is the last point's.
- `time_to` returns the x position (e.g. seconds since the start of the
  series), not the distance from `from`. For a duration, write
  `time_to(series.loaded, 90, from=t0) - t0`.
- Arithmetic between two series needs the same x values (after unit
  conversion); otherwise the sample is an `error`.

### Units

Every value has a unit, and the checker infers the unit of every part of an
expression from the [unit catalogue](definitions.md#units):

- `+`, `-`, the point-by-point `min`/`max` and `clamp` need convertible
  units; the other operands are converted to the first one with a unit.
- `*` and `/` combine units: `bytes_received / load_time` is in `B/ms`,
  which converts to `MB/s`.
- `^` needs a constant integer exponent unless the base is dimensionless.
- A number without a unit adapts to its context: in `load_time + 100` the
  `100` is in the unit of `load_time`; in `bytes_received / 1000` it is a
  plain factor. Write `100[ms]` where the meaning matters.
- A saved derived metric declares its unit; the inferred unit must convert
  to it, and values are converted to it. Analysis views can convert results
  to any convertible unit with their `unit` field (FR-13).

Units are checked when an expression is validated and again per sample
when it is evaluated, because the x unit of a series can differ between
uploads. A sample whose inputs no longer fit is an `error` for that sample.

### Values

- Values are computed in 64-bit floats (series points are stored as 32-bit
  floats).
- **No value** propagates: any operation with no value yields no value. A
  non-finite result (division by zero, `log10` of a negative number) is no
  value, and so is a threshold that is never reached.
- A sample that lacks an input is **missing**, not no value: it is left out
  of analysis and counted in `excluded.missing`.
- Series points with a non-finite y count as missing points: reductions
  skip them and they are dropped from series results.

### Composition

Derived metrics may use other derived metrics, without cycles and at most
32 levels deep. `derived.<name>` always means the definition that metric
has **now**, so a change takes effect in every metric built on it. The web
UI rejects a change that would make a metric built on it invalid (check it
in advance with `validate` and `name`).

### Limits

An expression has at most 4096 bytes and 64 levels of nesting.

### Examples

| Expression | Meaning |
|---|---|
| `bytes_received / load_time` | Throughput (`B/ms`; save it as `MB/s`) |
| `derived.throughput * 8` | The same in bits (save as `Mbit/s`) |
| `max(series.fps)` | Peak frame rate of a sample |
| `mean(series.fps, from=2[s])` | Mean frame rate after the first two seconds |
| `time_to(series.fps, 55)` | When the frame rate first reaches 55 fps |
| `time_to(series.loaded, 90, from=t0) - t0` | Time from `t0` until 90 % is loaded |
| `auc(series.power, interp=step)` | Energy from a power series (`W·s`, convertible to `J`) |
| `rekey(series.fps, series.transferred)` | Frame rate over transferred bytes instead of time |
| `clamp(load_time, 0, 10[s])` | Load time capped at 10 s |
| `` `load-time (cold)` - `load-time (warm)` `` | Names with special characters |
