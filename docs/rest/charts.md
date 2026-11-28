# Charts

A chart is a graph of nodes that prepares frames and ends in one
visualization (FR-22, [ADR 0022](../adr/0022-chart-and-dashboard-designer.md)).
These endpoints run a chart spec and check one. Both are `POST` with a JSON
body, need a token of **any scope**, and also exist on the UI port.

Saved charts, dashboards and their MCP tools follow later
([#32](https://github.com/sraesch/track-record/issues/32)).

## The spec

A spec is JSON in format 1: `variables` and `nodes`. Each node has an `id`,
a `type`, its `input` (or named `inputs` for a match) and its settings. The
last node is the visualization. ADR 0022 ("Nodes", "Visualizations",
"Variables", "Spec" and "Format 1") defines every node type and setting.

This trend of `load_time` gives one line per case and device, with the
mean of each run:

```json
{
  "format": 1,
  "variables": [{"name": "branch", "type": "text", "default": "main"}],
  "nodes": [
    {"id": "runs", "type": "source", "metric": "load_time",
     "filter": {"suite": "nightly", "sut_branch": {"var": "branch"}}},
    {"id": "lines", "type": "group", "input": "runs", "by": ["case", "device.name"]},
    {"id": "per_run", "type": "group", "input": "lines", "by": ["run.id"]},
    {"id": "samples", "type": "concatenate", "input": "per_run"},
    {"id": "ordered", "type": "sort", "input": "samples", "by": [{"key": "run.started_at"}]},
    {"id": "stats", "type": "reduce", "input": "ordered", "functions": ["mean", "ci_low", "ci_high"]},
    {"id": "chart", "type": "line", "input": "stats",
     "x": "run.started_at", "y": "mean", "low": "ci_low", "high": "ci_high"}
  ]
}
```

A source takes a [run filter](analysis.md#run-filter) and a metric
expression in the [expression language](derived-metrics.md). It gives one
frame per sample, with the attributes of the sample's run, SUT version,
environment, case, client device and sample.

**Limits:**
- 100 nodes and 8 sources per graph.
- 2,000,000 samples per source.
- A budget of points that one run may hold, over all its sources and
  nodes: 10 million by default, set with `TR_CHART_MAX_POINTS`.

A run over a limit fails with a message that says how to stay below it:
narrow the filter of a source, or reduce the series in the metric
expression, e.g. `max(series.fps)`.

## Run a chart

```http
POST /api/v1/charts/run
```

Runs a spec that is not saved (FR-22, FR-23). All sources are read from
one snapshot of the database, so data uploaded or invalidated during the
run does not change what it sees.

Request body `RunChart`:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `spec` | object | required | The spec |
| `variables` | map name → value | `{}` | Values of the chart's variables. A missing value takes the variable's default; without a default the run is rejected |
| `until` | string | | Stops at this node and returns its output (a preview of a node). Problems of nodes it does not evaluate do not stop the run; without `until`, every node must be valid, also one the visualization does not read |
| `preview` | boolean | `false` | Each source reads only the latest run that matches its filter |
| `max_points` | integer | `2000` | Most points per line for display, at least 2. Only applies to a `line` whose `y` is a column. Each frame keeps the minimum and maximum of each bucket of its points. A gap in a bucket stays a gap, unless `max_points` is 2: then minimum and maximum take priority |
| `format` | `csv` \| `json` \| `parquet` | | Returns the data as a file instead of events, see [Export](#export). Not with `until`, `preview` or `max_points` |

The response is `application/x-ndjson`: one JSON event per line, each with
a `type`:

| `type` | When | Fields |
|---|---|---|
| `start` | First | `node` (whose output the result is), `partial`, `sources` (`[{id, samples}]`), `total_samples` |
| `progress` | While the sources load | `done`, `total` (samples over all sources) |
| `result` | Once, on success | `node`, `partial`, `tree` (`ChartTree`), `excluded` (`Excluded` per source id) |
| `done` | Last, on success | |
| `error` | Last, on failure | `code`, `detail`, and `node` and `setting` for an error of a node |

`partial` is `true` for a preview, and when `until` names a node other than
the visualization.

Request errors are problem details returned **before** the stream starts.
These include a spec that is not well-formed or not valid, a missing
variable, an unknown `until` node, a source whose filter is invalid, and
too many samples. An error later ends the stream with an `error` event.
That can be data that does not fit a node (code `chart_failed`, e.g. two
partners with one key in a match), or the budget of points. A stream that
ends with neither `done` nor `error` was cut off.

`ChartTree` is the output of the node: either `groups` or `frames`.

| Type | Fields |
|---|---|
| `ChartTree` | `groups` (`ChartGroup`[]) or `frames` (`ChartFrame`[]) |
| `ChartGroup` | `key` (`[{name, value, unit}]`: the attributes it was formed by; `value` is `null` for the frames without one), `tree` (`ChartTree`) |
| `ChartFrame` | `attributes` (name → `{value, unit}`), `columns` (`[{name, unit, dimension}]`), `points` (`ChartPoint`[]), `input` (the input of a match it came from) |
| `ChartPoint` | `values` (one per column, in the order of `columns`; `null` is no value), `attributes` (only those in which the point differs from its frame) |

- Attribute values keep their JSON type: parameters stay numbers, strings
  or booleans.
- Times are RFC 3339 text.
- `unit` is left out when an attribute has none.
- `dimension` is set on the `x` of a series (`time`, `bytes`, ...).

Example: the trend above for two runs.

```bash
curl -sN -X POST "$TR_URL/api/v1/charts/run" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d "{\"spec\": $(cat trend.json), \"variables\": {\"branch\": \"main\"}}"
```

```text
{"type":"start","node":"chart","partial":false,"sources":[{"id":"runs","samples":4}],"total_samples":4}
{"type":"progress","done":4,"total":4}
{"type":"result","node":"chart","partial":false,"tree":{"frames":[{"attributes":{"device.name":{"value":"pixel-8"},"metric":{"value":"load_time"},"param.pipeline":{"value":"classic"},"param.size_mb":{"value":10,"unit":"MB"},"scenario.name":{"value":"load-model"},"source":{"value":"runs"},"sut.branch":{"value":"main"},...},"columns":[{"name":"mean","unit":"s"},{"name":"ci_low","unit":"s"},{"name":"ci_high","unit":"s"},{"name":"n","unit":""},{"name":"n_no_value","unit":""}],"points":[{"values":[1.375,-0.2132755920218301,2.96327559202183,2.0,0.0],"attributes":{"run.id":{"value":"0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01"},"run.started_at":{"value":"2026-11-01T10:00:00+00:00"},...}},{"values":[1.375,...],"attributes":{...}}]}]},"excluded":{"runs":{"missing":0,"errors":0,"first_error":null}}}
{"type":"done"}
```

(`{...}` and `...` shortened.) Reading the stream in Python:

```python
import json, requests

with requests.post(f"{TR_URL}/api/v1/charts/run", json={"spec": spec}, stream=True,
                   headers={"Authorization": f"Bearer {TR_TOKEN}"}) as r:
    r.raise_for_status()
    for line in r.iter_lines():
        event = json.loads(line)
        if event["type"] == "result":
            tree = event["tree"]
        elif event["type"] == "error":
            raise RuntimeError(event["detail"])
```

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `format` with `until`, `preview` or `max_points`; `max_points` below 2 |
| `422` | `invalid_chart` | The spec is not well-formed or not valid (e.g. a field a source filter does not have, such as `limit` or `cursor`), a variable has no value or one of the wrong type, `until` names no node. `detail` lists the problems; use [validate](#validate-a-chart) to get them per node and setting |
| `422` | `invalid_expression` | The metric expression of a source is invalid |
| `422` | `validation_failed` | A source selects more than 2,000,000 samples, or its filter or options do not fit |
| `422` | `invalid_request_body` | Unknown field or wrong type in the body |

### Export

With `format`, the response is the data that reaches the visualization as
a file (FR-20), with one row per point. Its columns are:

- every attribute of the frames and points, by name (`run.id`,
  `device.name`, `param.size_mb`, ...);
- `input`, if a frame came from a match;
- the columns of the frames (`x`, `y`, `mean`, ...), in the order they
  first appear.

An attribute or column with a unit gets a `unit(<name>)` column after it
(`unit(param.size_mb)`, `unit(mean)`). No attribute or chart column can
have such a name.

| Format | `Content-Type` | File name |
|---|---|---|
| `csv` | `text/csv; charset=utf-8` | `track-record-chart.csv` |
| `json` | `application/x-ndjson` (one object per row) | `track-record-chart.ndjson` |
| `parquet` | `application/vnd.apache.parquet` | `track-record-chart.parquet` |

- A column's type is the type all its values share: booleans, integers
  (that fit 64 bits), numbers (exact as 64-bit floats), text, or times
  (RFC 3339 text).
- Values never lose their type or digits. A column of lists, of integers
  that a float cannot hold exactly, or of mixed types (the number `10` in
  one case, the text `"10"` in another) holds JSON values. In NDJSON they
  are native, in CSV and Parquet JSON text: `10`, `"10"`, `["a"]`.
- An empty cell (`null` in JSON) is no value, or an attribute the point
  does not have. In CSV, a text column that has an empty text holds JSON
  text, so that `""` stays apart from an empty cell.
- A chart column that has the name of an attribute fails the export; rename
  it in the node that makes it.

```bash
curl -s -X POST "$TR_URL/api/v1/charts/run" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d "{\"spec\": $(cat trend.json), \"format\": \"csv\"}" -o trend.csv
```

Errors before the start are problem details, as for events. An error after
the start aborts the transfer, so a cut-off file is never complete.

## Validate a chart

```http
POST /api/v1/charts/validate
```

Checks a spec without running it:
- **well-formed:** known node types and settings, unique ids, inputs that
  exist, no cycle, exactly one visualization, within the limits. Only a
  well-formed spec can be saved.
- **valid:** the expressions, attributes, variables, columns, units and
  group levels fit. Only a valid spec can run.

Settings that are variables are checked with their defaults. The answer is
always `200`; problems are part of it.

Request body `ValidateChart`: `spec` (object).

Response `ChartValidation`:

| Field | Type | Meaning |
|---|---|---|
| `well_formed` | boolean | The spec can be saved |
| `valid` | boolean | The spec can run |
| `problems` | `ChartProblem`[] | `node`, `setting` (e.g. `metric`, `filter.sut_branch`, `functions[1]`, `variables[0].default`), `message`, and `start`/`end`: byte offsets inside the setting's expression, as the [expression validation](derived-metrics.md#validate-an-expression) gives them |
| `nodes` | map node id → `NodeShape` | The output of each node whose inputs could be checked; empty when the spec is not well-formed |

`NodeShape`:
- `levels`: group levels, outermost first. Each is
  `{"kind": "group", "by", "except"}` or `{"kind": "match", "inputs"}`.
- `columns`: `[{name, unit}]`. `unit` is `null` when it is known only when
  the chart runs.
- `attributes`: the attributes frames and points can have. `param.*` and
  `asset.*` stand for every parameter and asset.

```bash
curl -s -X POST "$TR_URL/api/v1/charts/validate" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"spec": {"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time + nope"},
        {"id": "chart", "type": "table", "input": "runs", "columns": ["y"]}]}}'
```

```json
{
  "well_formed": true,
  "valid": false,
  "problems": [
    {"node": "runs", "setting": "metric", "message": "unknown metric nope", "start": 12, "end": 16}
  ],
  "nodes": {}
}
```

| Status | Code | When |
|---|---|---|
| `422` | `invalid_request_body` | No `spec`, or an unknown field |
