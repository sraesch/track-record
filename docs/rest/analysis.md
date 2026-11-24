# Analysis

Read-only views over many runs (FR-12 to FR-20): aggregates, trends,
comparisons, series overlays and bands, and exports. Every endpoint is a
`POST` with a JSON **view spec** and works with a token of any scope. The
same specs are the tool parameters of the MCP server.

Each view spec names its metric as an [expression](derived-metrics.md#expression-language):
an uploaded metric (`load_time`), a series (`series.fps`), a derived metric
(`derived.throughput`) or any expression (`max(series.fps)`).

## Run filter

The analysis views select runs with a `filter` object; `GET /runs` takes the
same fields as query parameters (FR-11). All given conditions must hold.

| Field | Type | Meaning |
|---|---|---|
| `run_id` | UUID[] | Only these runs |
| `suite` | string \| null | Suite name |
| `sut_commit`, `sut_branch`, `sut_tag`, `sut_build_config` | string \| null | SUT version fields, exact match |
| `started_after` | timestamp \| null | Measured start at or after this time |
| `started_before` | timestamp \| null | Measured start before this time |
| `status` | (`running` \| `completed` \| `failed` \| `aborted`)[] | Any of these statuses |
| `triggered_by` | `nightly` \| `manual` \| `ci` \| null | Trigger |
| `shared_environment_id` | integer \| null | Shared environment |
| `client_device` | string \| null | Client device name **after aliases** |
| `scenario` | string \| null | Scenario name |
| `asset` | string \| null | Asset name |
| `param` | string[] | Parameter filters `name=value`; all must hold |
| `metric` | string \| null | Metric name with a measurement or series in the run |
| `tag` | string[] | The run has **all** these tags |
| `validity` | `valid` \| `invalid` \| `all` | Default `valid`: invalid runs are excluded unless asked for (FR-9) |
| `limit`, `cursor` | | Paging of `GET /runs` only; in an analysis they are a `400 bad_request` |

**Case execution filters.** `client_device`, `scenario`, `asset`, `param`
and `metric` match runs with at least one **current** (not superseded) case
execution that satisfies all of them. In the analysis views they also
restrict which case executions are used, not only which runs.

**Parameter values keep their JSON type.** The value after `=` is read as
JSON: `size=10` matches the number 10, `size="10"` the string `"10"`,
`progressive=true` the boolean. Anything that is not a JSON scalar is taken
as a string, so `codec=draco` matches the string `"draco"`. Parameter and
device filters compare the values **after aliases**. In a query string,
encode the value: `param=codec%3D%22draco%22`.

An empty filter (`{}` or no `filter`) selects all valid runs.

## Common fields

| Field | Type | Default | Meaning |
|---|---|---|---|
| `filter` | `RunFilter` | `{}` | Which runs |
| `metric` | string | required | The metric expression |
| `unit` | string \| null | the expression's unit | Convert all values to this unit (FR-13) |
| `include_warmup` | boolean | `false` | Include warmup samples (FR-12) |
| `percentiles` | number[] \| null | `[5, 25, 75, 95]` | Percentiles in `[0, 100]`, at most 20 |
| `confidence` | number | `0.95` | Level of the confidence interval of the mean, in `(0, 1)` |

Without `unit`, values are in the unit the expression has with the current
metric units, e.g. `ms` for `load_time`.

**Grouping.** Samples are grouped by **run × case × client device**. The
case is the scenario version, the assets and the parameters after aliases;
the device is the name after aliases. Samples of different runs are never
pooled (FR-16).

**Missing and no value.** Samples that lack an input of the expression, or
whose inputs do not fit it, are left out and counted in `excluded`. Samples
for which the expression has **no value** (FR-12a) are kept and counted in
`n_no_value`.

**Size.** One analysis touches at most 2,000,000 samples; narrow the filter
otherwise (`422 validation_failed`).

### Shared result types

`MetricInfo`:

| Field | Type | Meaning |
|---|---|---|
| `expression` | string | The metric expression as requested |
| `kind` | `scalar` \| `series` | |
| `unit` | string | Unit of all values |

`RunRef` (a run, briefly):

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | |
| `started_at` | timestamp | Measured start |
| `status` | string | Run status |
| `is_valid` | boolean | |
| `sut_version` | `SutVersion` | `id`, `commit`, `branch`, `build_config`, `tag` |
| `shared_environment_id` | integer | |

`CaseInfo` (a case):

| Field | Type | Meaning |
|---|---|---|
| `scenario`, `scenario_version`, `scenario_version_id` | | The scenario version |
| `assets` | `AssetRef`[] | |
| `parameters` | object | Parameters **after aliases** (in a comparison: only those in the matching key) |
| `parameter_units` | object | Units of numeric parameters |
| `test_case_ids` | integer[] | The test cases that form this case; several when aliases merge parameter values |

`Stats` (across the samples of one group):

| Field | Type | Meaning |
|---|---|---|
| `n` | integer | Samples with a value |
| `n_no_value` | integer | Samples without a value; not part of the statistics below |
| `mean`, `median`, `min`, `max` | number \| null | |
| `stddev` | number \| null | Sample standard deviation (n − 1) |
| `percentiles` | `{p, value}`[] | The requested percentiles, linearly interpolated |
| `ci_low`, `ci_high` | number \| null | Confidence interval of the mean (Student's t) at `confidence` |

Values are `null` when they cannot be computed, e.g. without samples with a
value, or the standard deviation and interval with fewer than two.

`Excluded`:

| Field | Type | Meaning |
|---|---|---|
| `missing` | integer | Samples without an input of the expression (not measured) |
| `errors` | integer | Samples whose inputs do not fit the expression (e.g. units) |
| `first_error` | string \| null | The first such error, for diagnosis |

## Aggregates

```http
POST /api/v1/analysis/aggregates
```

Statistics per run, case and client device (FR-12). The metric must be a
**scalar** (use `max(series.fps)` and the like for series).

Request body `AggregatesRequest`: the [common fields](#common-fields).

Response `AggregatesResponse`:

| Field | Type | Meaning |
|---|---|---|
| `metric` | `MetricInfo` | |
| `groups` | `AggregateGroup`[] | One per run, case and device, in run order (oldest first) |
| `excluded` | `Excluded` | |

`AggregateGroup`: `run` (`RunRef`), `case` (`CaseInfo`), `client_device`
(name after aliases), `case_execution_ids` (UUID[]), `stats` (`Stats`).

```bash
curl -s -X POST "$TR_URL/api/v1/analysis/aggregates" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "filter": {"suite": "web-viewer", "sut_branch": "main"},
    "metric": "load_time",
    "unit": "s",
    "percentiles": [50, 90]
  }'
```

```json
{
  "metric": {"expression": "load_time", "kind": "scalar", "unit": "s"},
  "groups": [
    {
      "run": {
        "id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
        "started_at": "2026-11-20T02:00:00Z",
        "status": "completed",
        "is_valid": true,
        "sut_version": {"id": 1, "commit": "4f2a9c1", "branch": "main", "build_config": "release", "tag": null},
        "shared_environment_id": 1
      },
      "case": {
        "scenario": "load-model",
        "scenario_version": 1,
        "scenario_version_id": 1,
        "assets": [{"id": 1, "name": "engine.glb", "version": "3", "content_hash": null}],
        "parameters": {"bandwidth": 100, "codec": "draco", "progressive": true},
        "parameter_units": {"bandwidth": "Mbit/s"},
        "test_case_ids": [1]
      },
      "client_device": "pixel-8-lab-1",
      "case_execution_ids": ["5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11"],
      "stats": {
        "n": 2,
        "n_no_value": 0,
        "mean": 1.5555,
        "median": 1.5555,
        "stddev": 0.007071067811865481,
        "min": 1.5505,
        "max": 1.5605,
        "percentiles": [{"p": 50.0, "value": 1.5555}, {"p": 90.0, "value": 1.5595}],
        "ci_low": 1.4919689763191266,
        "ci_high": 1.6190310236808731
      }
    }
  ],
  "excluded": {"missing": 0, "errors": 0, "first_error": null}
}
```

(One group shown; the warmup sample is excluded, so `n` is 2.)

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `limit`/`cursor` in the filter, percentiles or confidence out of range |
| `422` | `invalid_expression` | The metric expression is invalid |
| `422` | `validation_failed` | A series metric, more than 2,000,000 samples |

## Trend

```http
POST /api/v1/analysis/trend
```

One case, metric and client device across runs (FR-18), with markers where
the SUT version, the shared environment or the device details change, and
the annotations that belong to it.

Request body `TrendRequest`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `test_case_id` | integer | yes | **Any** test case of the case. Test cases whose parameters are equal after aliases belong to the same case; other scenario versions never do |
| `client_device` | string | yes | Device name after aliases |
| `metric` | string | yes | A scalar metric expression |
| `filter` | `RunFilter` | no | Further restrictions, e.g. a branch or a time range |
| `order_by` | `started_at` \| `sut_version` | no | `started_at` (default): by measured start. `sut_version`: grouped by SUT version, in the order of each version's first run, then by start |
| `reference_run_id` | UUID \| null | no | Normalize to the mean of this run (FR-13); it must be one of the trend's runs |
| `unit`, `include_warmup`, `percentiles`, `confidence` | | no | [Common fields](#common-fields) |

Response `TrendResponse`:

| Field | Type | Meaning |
|---|---|---|
| `metric` | `MetricInfo` | |
| `case` | `CaseInfo` | The case |
| `client_device` | string | Device name after aliases |
| `points` | `TrendPoint`[] | One per run, in the requested order |
| `annotations` | `Annotation`[] | Annotations on the trend's runs, on the case's test cases, and point-in-time annotations between the first and the last run's start |
| `excluded` | `Excluded` | |

`TrendPoint`:

| Field | Type | Meaning |
|---|---|---|
| `run` | `RunRef` | |
| `stats` | `Stats` | |
| `relative_to_reference` | number \| null | Mean divided by the reference run's mean, if a reference is set (and both means exist and the reference is not 0) |
| `device_environment_ids` | integer[] | Device details used in this run |
| `markers` | string[] | Changes against the previous point: `sut_version_changed`, `shared_environment_changed`, `device_details_changed`. Device detail changes are marked; they never split a trend |

```bash
curl -s -X POST "$TR_URL/api/v1/analysis/trend" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "test_case_id": 1,
    "client_device": "pixel-8-lab-1",
    "metric": "load_time",
    "filter": {"sut_branch": "main"},
    "reference_run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
    "percentiles": [50]
  }'
```

```json
{
  "metric": {"expression": "load_time", "kind": "scalar", "unit": "ms"},
  "case": {
    "scenario": "load-model", "scenario_version": 1, "scenario_version_id": 1,
    "assets": [{"id": 1, "name": "engine.glb", "version": "3", "content_hash": null}],
    "parameters": {"bandwidth": 100, "codec": "draco", "progressive": true},
    "parameter_units": {"bandwidth": "Mbit/s"},
    "test_case_ids": [1]
  },
  "client_device": "pixel-8-lab-1",
  "points": [
    {
      "run": {"id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01", "started_at": "2026-11-20T02:00:00Z", "...": "..."},
      "stats": {"n": 2, "n_no_value": 0, "mean": 1555.5, "median": 1555.5, "stddev": 7.0710678118654755,
                "min": 1550.5, "max": 1560.5, "percentiles": [{"p": 50.0, "value": 1555.5}],
                "ci_low": 1491.9689763191268, "ci_high": 1619.0310236808732},
      "relative_to_reference": 1.0,
      "device_environment_ids": [5],
      "markers": []
    },
    {
      "run": {"id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e02", "started_at": "2026-11-21T02:00:00Z", "...": "..."},
      "stats": {"n": 2, "n_no_value": 0, "mean": 1435.5, "median": 1435.5, "stddev": 7.0710678118654755,
                "min": 1430.5, "max": 1440.5, "percentiles": [{"p": 50.0, "value": 1435.5}],
                "ci_low": 1371.9689763191268, "ci_high": 1499.0310236808732},
      "relative_to_reference": 0.922854387656702,
      "device_environment_ids": [7],
      "markers": ["device_details_changed", "sut_version_changed"]
    }
  ],
  "annotations": [
    {
      "id": "6f2a9763-671e-493d-8407-e3af3a52755d",
      "run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
      "test_case_id": null,
      "occurred_at": null,
      "body": "Render server was restarted before this run; first samples are slow.",
      "created_at": "2026-11-23T19:57:13.704329Z",
      "updated_at": "2026-11-23T19:57:13.714812Z"
    }
  ],
  "excluded": {"missing": 0, "errors": 0, "first_error": null}
}
```

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | As for aggregates |
| `404` | `not_found` | No such test case |
| `422` | `invalid_expression` | The metric expression is invalid |
| `422` | `validation_failed` | A series metric; the reference run is not in the trend |

## Compare

```http
POST /api/v1/analysis/compare
```

A side-by-side comparison along one **dimension** (FR-16): runs, client
devices or case parameters. Everything that is not compared forms the
**matching key** of a row; each column is one value of the dimension. The
result is **streamed as NDJSON** (NFR-7), so large comparisons show
progress.

Request body `CompareRequest`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `dimension` | `run` \| `client_device` \| `parameters` | yes | What the columns are |
| `metric` | string | yes | A scalar metric expression |
| `filter` | `RunFilter` | no | The runs to compare: one run or a filtered set |
| `parameters` | string[] | for `parameters` | The compared parameters |
| `exclude_from_key` | string[] | no | Parameters taken out of the matching key, so rows that differ only in them are merged (their values are reported per cell run) |
| `values` | JSON[] \| null | no | The columns to show, in this order. Default: all values found (runs by start time, other values sorted) |
| `reference` | JSON \| null | no | The reference column. Default: the first value. Must be one of `values` |
| `unit`, `include_warmup`, `percentiles`, `confidence` | | no | [Common fields](#common-fields) |

Values of the dimension:

| Dimension | Value | Example |
|---|---|---|
| `run` | Run id as string | `"0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01"` |
| `client_device` | Device name (aliases applied) | `"pixel-8-lab-1"` |
| `parameters` | Object of the compared parameters (aliases applied, JSON types kept) | `{"codec": "draco"}` |

The response is `application/x-ndjson`: one JSON event per line, each with
a `type`:

| `type` | When | Fields |
|---|---|---|
| `start` | First | `metric` (`MetricInfo`), `dimension`, `values` (the columns), `reference`, `total_samples` |
| `progress` | While evaluating | `done`, `total` (samples) |
| `row` | Once per matching key | `case` (`CaseInfo` without the compared and excluded parameters), `client_device` (`null` when devices are compared), `cells` |
| `done` | Last, on success | `rows`, `excluded` (`Excluded`) |
| `error` | Last, on failure | `code`, `detail` |

Request errors are returned as normal problem details **before** the stream
starts. An error during evaluation ends the stream with an `error` event; a
stream that ends with neither `done` nor `error` was cut off.

`CompareCell` (only columns with results, in column order):

| Field | Type | Meaning |
|---|---|---|
| `value` | JSON | The dimension value of this column |
| `is_reference` | boolean | This is the reference column |
| `runs` | `CellRun`[] | One aggregate per run, oldest first. The **last** is the one compared. Samples of different runs are never pooled |
| `difference` | object \| null | Against the reference cell of the row: `absolute` (mean − reference mean), `relative` (`absolute` / \|reference mean\|; `null` if that is 0), `ratio` (mean / reference mean, FR-13). `null` for the reference itself, when the row has no reference cell, or when not computable |
| `not_computable` | boolean | This cell or the reference has no value, so no difference is computed (FR-12a) |
| `sut_version_differs` | boolean | The compared run has another SUT version than the reference's |
| `shared_environment_differs` | boolean | The compared run has another shared environment than the reference's |

`CellRun`: `run` (`RunRef`), `stats` (`Stats`), `excluded_parameters`
(values of the parameters taken out of the key, as objects).

Example: two devices in one run.

```bash
curl -sN -X POST "$TR_URL/api/v1/analysis/compare" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "filter": {"run_id": ["0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e02"]},
    "dimension": "client_device",
    "metric": "load_time",
    "reference": "pixel-8-lab-1",
    "percentiles": [50]
  }'
```

```text
{"type":"start","metric":{"expression":"load_time","kind":"scalar","unit":"ms"},"dimension":"client_device","values":["iphone-15-lab-2","pixel-8-lab-1"],"reference":"pixel-8-lab-1","total_samples":4}
{"type":"progress","done":4,"total":4}
{"type":"row","case":{...},"client_device":null,"cells":[{"value":"iphone-15-lab-2","is_reference":false,"runs":[{"run":{...},"stats":{"n":2,"mean":1635.5,...},"excluded_parameters":[]}],"difference":{"absolute":200.0,"relative":0.13932427725531174,"ratio":1.1393242772553118},"not_computable":false,"sut_version_differs":false,"shared_environment_differs":false},{"value":"pixel-8-lab-1","is_reference":true,"runs":[{"run":{...},"stats":{"n":2,"mean":1435.5,...},"excluded_parameters":[]}],"difference":null,"not_computable":false,"sut_version_differs":false,"shared_environment_differs":false}]}
{"type":"done","rows":1,"excluded":{"missing":0,"errors":0,"first_error":null}}
```

(`{...}` shortened.) Reading the stream line by line in Python:

```python
import json, requests

with requests.post(f"{TR_URL}/api/v1/analysis/compare", json=spec, stream=True,
                   headers={"Authorization": f"Bearer {TR_TOKEN}"}) as r:
    r.raise_for_status()
    for line in r.iter_lines():
        event = json.loads(line)
        if event["type"] == "row":
            ...
        elif event["type"] == "error":
            raise RuntimeError(event["detail"])
```

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `dimension: parameters` without `parameters`, options out of range, `limit`/`cursor` in the filter |
| `422` | `invalid_expression` | The metric expression is invalid |
| `422` | `validation_failed` | A series metric, a reference that is not one of the values, too many samples |

## Series overlay

```http
POST /api/v1/analysis/series/overlay
```

The series of many samples, for drawing them on top of each other (FR-19):
raw (downsampled for display) or resampled onto a common x grid.

Request body `SeriesViewRequest`:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `metric` | string | required | A **series**: `series.<name>`, a derived series metric or a series expression |
| `filter` | `RunFilter` | `{}` | Which runs |
| `unit` | string \| null | the series' unit | Unit of y |
| `x_unit` | string \| null | x unit of the most recent series | Unit of x; all series are converted to it |
| `include_warmup` | boolean | `false` | |
| `grid` | `GridSpec` \| null | none | Resample onto a common grid |
| `interp` | `linear` \| `step` | `linear` | Interpolation for resampling |
| `max_series` | integer \| null | 500 | At most this many series (1 to 5000) |
| `max_points` | integer \| null | 2000 | Without a grid: downsample each series to at most this many points (at least 2) |

`GridSpec`: `points` evenly spaced x values from `from` to `to`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `points` | integer \| null | 200 | Number of grid points, 2 to 10000 |
| `from` | number \| null | smallest x of the series | Start of the grid, in `x_unit` |
| `to` | number \| null | largest x of the series | End of the grid, in `x_unit` |

Response `OverlayResponse`:

| Field | Type | Meaning |
|---|---|---|
| `metric` | `MetricInfo` | |
| `x_unit`, `x_dimension` | string | Unit and meaning of x |
| `grid` | number[] \| null | The common grid, if one was requested |
| `series` | `OverlaySeries`[] | One per sample |
| `truncated` | boolean | More series matched than `max_series` |
| `excluded` | `Excluded` | |

`OverlaySeries`: `run` (`RunRef`), `case` (`CaseInfo`), `client_device`,
`case_execution_id`, `sample_id`, `repetition`, `is_warmup`, and:

| Field | Type | Meaning |
|---|---|---|
| `x` | number[] \| null | x values; `null` when resampled onto the grid |
| `y` | (number \| null)[] | y values; `null` where the series has no value at a grid point |

```bash
curl -s -X POST "$TR_URL/api/v1/analysis/series/overlay" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "filter": {"run_id": ["0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01"]},
    "metric": "series.fps",
    "grid": {"points": 5, "from": 0, "to": 2}
  }'
```

```json
{
  "metric": {"expression": "series.fps", "kind": "series", "unit": "fps"},
  "x_unit": "s",
  "x_dimension": "time",
  "grid": [0.0, 0.5, 1.0, 1.5, 2.0],
  "series": [
    {
      "run": {"id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01", "...": "..."},
      "case": {"scenario": "load-model", "...": "..."},
      "client_device": "pixel-8-lab-1",
      "case_execution_id": "5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11",
      "sample_id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11",
      "repetition": 1,
      "is_warmup": false,
      "x": null,
      "y": [0.0, 25.0, 55.0, 60.0, 59.5]
    }
  ],
  "truncated": false,
  "excluded": {"missing": 0, "errors": 0, "first_error": null}
}
```

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `max_series`, `max_points` or the grid out of range, `from` > `to`, `limit`/`cursor` in the filter |
| `422` | `invalid_expression` | The metric expression is invalid |
| `422` | `validation_failed` | A scalar metric; an `x_unit` the series cannot be converted to |

## Series aggregate

```http
POST /api/v1/analysis/series/aggregate
```

Series of the samples of each run, case and device, resampled onto a grid
and aggregated per grid point: percentile bands, e.g. a median with an
interquartile band (FR-19).

Request body: `SeriesViewRequest` as for the [overlay](#series-overlay),
with these differences:

- `percentiles` (default `[25, 50, 75]`, at most 20) are computed per grid
  point;
- the series are always resampled: `grid` is optional, and without
  `from`/`to` the grid covers the series of each group;
- `max_series` and `max_points` are not allowed (`400`).

Response `SeriesAggregateResponse`:

| Field | Type | Meaning |
|---|---|---|
| `metric` | `MetricInfo` | |
| `x_unit`, `x_dimension` | string | |
| `groups` | `SeriesGroup`[] | One per run, case and device |
| `excluded` | `Excluded` | |

`SeriesGroup`:

| Field | Type | Meaning |
|---|---|---|
| `run`, `case`, `client_device` | | The group |
| `n_series` | integer | Number of series in the group |
| `x` | number[] | The grid |
| `n` | integer[] | Number of series with a value at each grid point |
| `percentiles` | `{p, values}`[] | Per percentile, one value per grid point (`null` where no series has a value) |

```bash
curl -s -X POST "$TR_URL/api/v1/analysis/series/aggregate" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"filter": {"run_id": ["0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e02"]}, "metric": "series.fps",
       "grid": {"points": 3}, "percentiles": [50]}'
```

```json
{
  "metric": {"expression": "series.fps", "kind": "series", "unit": "fps"},
  "x_unit": "s",
  "x_dimension": "time",
  "groups": [
    {
      "run": {"id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e02", "...": "..."},
      "case": {"scenario": "load-model", "...": "..."},
      "client_device": "iphone-15-lab-2",
      "n_series": 2,
      "x": [0.0, 1.0, 2.0],
      "n": [2, 2, 2],
      "percentiles": [{"p": 50.0, "values": [0.0, 55.79999923706055, 59.5]}]
    }
  ],
  "excluded": {"missing": 0, "errors": 0, "first_error": null}
}
```

Errors as for the [overlay](#series-overlay).

## Export

```http
POST /api/v1/analysis/export
```

Exports per-sample values or aggregates of one scalar metric as CSV, NDJSON
or Parquet (FR-20). The file is streamed as it is produced (NFR-7).

Request body `ExportRequest`:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `format` | `csv` \| `json` \| `parquet` | required | File format; `json` is NDJSON (one object per line) |
| `level` | `samples` \| `aggregates` | `samples` | One row per sample, or one per run, case and device |
| `metric` | string | required | A scalar metric expression |
| `percentiles` | number[] \| null | `[5, 25, 75, 95]` | For `level: aggregates` only (`400` otherwise) |
| `filter`, `unit`, `include_warmup`, `confidence` | | | [Common fields](#common-fields) |

Response: the file, with

| Format | `Content-Type` | File name |
|---|---|---|
| `csv` | `text/csv; charset=utf-8` | `track-record-export.csv` |
| `json` | `application/x-ndjson` | `track-record-export.ndjson` |
| `parquet` | `application/vnd.apache.parquet` | `track-record-export.parquet` |

(`Content-Disposition: attachment; filename="track-record-export.<ext>"`.)

Columns of `level: samples`:

| Column | Type | Meaning |
|---|---|---|
| `run_id` | string | |
| `run_started_at` | string | RFC 3339 |
| `run_is_valid` | boolean | |
| `sut_commit`, `sut_branch`, `sut_build_config`, `sut_tag` | string | Empty/null if not set |
| `shared_environment_id` | integer | |
| `case_execution_id` | string | |
| `test_case_id` | integer | |
| `scenario` | string | |
| `scenario_version` | integer | |
| `assets` | string | `name@version` or `name#hash`, separated by `;` |
| `parameters` | string | Parameters after aliases, as JSON text |
| `client_device` | string | Name after aliases |
| `device_environment_id` | integer | |
| `sample_id` | string | |
| `repetition` | integer | |
| `is_warmup` | boolean | |
| `metric` | string | The expression |
| `unit` | string | |
| `value` | number | Empty/null when the sample has no value |
| `has_value` | boolean | `false` for no value (FR-12a) |

Columns of `level: aggregates`: `run_id`, `run_started_at`,
`run_is_valid`, `sut_commit`, `sut_branch`, `sut_build_config`, `sut_tag`,
`shared_environment_id`, `scenario`, `scenario_version`, `assets`,
`parameters` (after aliases), `client_device`, `metric`, `unit`, `n`,
`n_no_value`, `mean`, `median`, `stddev`, `min`, `max`, `ci_low`,
`ci_high`, then one column per percentile named `p<percentile>` (`p50`,
`p95`, `p99.9`).

Format details:

- **CSV**: a header row, RFC 4180 quoting, empty cells for missing values.
- **NDJSON**: one JSON object per row, `null` for missing values.
- **Parquet**: typed columns (strings, 64-bit integers, doubles, booleans);
  one row group per chunk of 2000 samples.

```bash
curl -s -X POST "$TR_URL/api/v1/analysis/export" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"filter": {"run_id": ["0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01"]}, "metric": "load_time", "format": "csv"}' \
  -o load_time.csv
```

```text
run_id,run_started_at,run_is_valid,sut_commit,sut_branch,sut_build_config,sut_tag,shared_environment_id,case_execution_id,test_case_id,scenario,scenario_version,assets,parameters,client_device,device_environment_id,sample_id,repetition,is_warmup,metric,unit,value,has_value
0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01,2026-11-20T02:00:00+00:00,true,4f2a9c1,main,release,,1,5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11,1,load-model,1,engine.glb@3,"{""bandwidth"":100,""codec"":""draco"",""progressive"":true}",pixel-8-lab-1,5,9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11,1,false,load_time,ms,1550.5,true
0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01,2026-11-20T02:00:00+00:00,true,4f2a9c1,main,release,,1,5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11,1,load-model,1,engine.glb@3,"{""bandwidth"":100,""codec"":""draco"",""progressive"":true}",pixel-8-lab-1,5,9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c12,2,false,load_time,ms,1560.5,true
```

Request errors are answered with problem details before the file starts.
An error after the start aborts the transfer, so the client sees an
incomplete response rather than a short but valid-looking file.

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `percentiles` with `level: samples`, options out of range, `limit`/`cursor` in the filter |
| `422` | `invalid_expression` | The metric expression is invalid |
| `422` | `validation_failed` | A series metric, too many samples |
