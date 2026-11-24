# Ingestion

Uploading benchmark results (FR-1 to FR-5). All endpoints on this page need
a token with the **`write`** scope.

Upload is **incremental**, so partial data survives a harness crash:

1. [Create the run](#create-a-run) when the benchmark starts. It is
   `running`.
2. [Add a case execution](#add-a-case-execution) for each test case and
   device, with its samples inline or [one sample](#add-a-sample) and
   [one series](#add-a-series) at a time as they finish.
3. [Upload attachments](#upload-an-attachment) (logs, screenshots) at any
   point.
4. [Close the run](#close-a-run) as `completed` or `failed`.

Every result gets a **client-generated UUID**, and every write is
[idempotent](conventions.md#idempotent-writes): `201` when it stored
something new, `200` for an identical retry, `409 conflict` for different
content under an existing id.

All write endpoints return an `IngestResponse`:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | The id from the path |
| `created` | boolean | `false` if everything already existed with identical content (a retry); for [close](#close-a-run), `false` if the run already had that status |

## Metric definitions in uploads

Every request that sends measurements or series also sends the
definitions of the metrics it uses, in `metrics` (metric name →
`MetricInput`). Each metric used in `measurements` or `series` must be in
the `metrics` of the **same request**; otherwise `422 validation_failed`
(`metric "fps" is used but not defined in \`metrics\``).

`MetricInput`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `unit` | string | yes | Unit symbol, preferably from the [unit catalogue](definitions.md#units); `""` for dimensionless |
| `value_type` | `float` \| `integer` \| `boolean` | yes | Type of the measured values |
| `direction` | `higher_is_better` \| `lower_is_better` | yes | Which way is better |
| `description` | string | no | Free text, default `""` |

A metric that does not exist yet is created with this definition. A metric
that exists is **never changed** by an upload
([ADR 0018](../adr/0018-metrics-without-versions.md)):

- `direction` and `description` of an existing metric are ignored;
- `unit` and `value_type` must be the ones it has. Otherwise the request is
  rejected with `409 metric_definition_conflict`, because the values would
  be stored under the wrong unit.

Metrics are changed (and renamed) only in the web UI. If someone changes a
metric's unit there, uploads have to send the new unit from then on; read
the current definitions with [`GET /metrics`](definitions.md#metrics).

Metric names must not be empty. They are case-sensitive.

## Create a run

```http
PUT /api/v1/runs/{run_id}
```

Creates a run with the client-generated `run_id`. The run starts as
`running`.

Request body `CreateRun`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `suite` | `NamedDefinition` | yes | The suite: `name` (non-empty) and its free-form JSON `definition` |
| `sut_version` | `SutVersionInput` | yes | Version of the system under test |
| `shared_environment` | object | yes | Details of the shared environment (servers, backends, ...); any JSON object, `{}` if there is none |
| `triggered_by` | `nightly` \| `manual` \| `ci` | yes | What started the run |
| `started_at` | timestamp | yes | **Measured** start time from the harness. Trends and time filters use it |
| `tags` | string[] | no | Tags the run is created with. Trimmed, deduplicated, must not be empty |

`NamedDefinition`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | Name of the suite or scenario |
| `definition` | any JSON | yes | Its definition. Identical definitions map to the same version; a changed definition creates a new version (FR-7) |

`SutVersionInput`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `commit` | string | yes | Commit (non-empty) |
| `branch` | string \| null | no | Branch |
| `build_config` | string \| null | no | Build configuration, e.g. `release` |
| `tag` | string \| null | no | Version tag, e.g. `v2.4.0` |

Identical SUT versions and identical shared environments are stored once
(the environment by the SHA-256 fingerprint of its details).

**Retry semantics.** A retry is identical when suite version, SUT version,
shared environment, `triggered_by` and `started_at` are equal. `tags` are
not compared (they may have been edited since) and are not applied again.

Example:

```bash
curl -s -X PUT "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "suite": {"name": "web-viewer", "definition": {"scenarios": ["load-model"]}},
    "sut_version": {"commit": "4f2a9c1", "branch": "main", "build_config": "release"},
    "shared_environment": {"server": "render-01", "backend_version": "2.4.1"},
    "triggered_by": "nightly",
    "started_at": "2026-11-20T02:00:00Z",
    "tags": ["nightly"]
  }'
```

```json
{"id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01", "created": true}
```

| Status | Code | When |
|---|---|---|
| `201` | | Created |
| `200` | | The run existed with this content |
| `409` | `conflict` | The run exists with other content |
| `422` | `validation_failed`, `invalid_request_body` | Empty name or commit, empty tag, schema violation |

## Add a case execution

```http
PUT /api/v1/runs/{run_id}/case-executions/{case_execution_id}
```

Adds a case execution (one test case on one client device) to a run,
optionally with its samples and their series. The whole request is one
transaction.

A **new** case execution needs a `running` run
(`409 run_not_running` otherwise). Only [corrections](#corrections) may be
added to a run that has finished. A retry of a stored case execution
succeeds after the run was closed, and samples and series can still be
added to existing case executions then.

Request body `CreateCaseExecution`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `test_case` | `TestCaseInput` | yes | Scenario, assets and parameters |
| `client_device` | `ClientDeviceInput` | yes | The device it ran on |
| `started_at` | timestamp \| null | no | Measured start time |
| `ended_at` | timestamp \| null | no | Measured end time; not before `started_at` |
| `supersedes_id` | UUID \| null | no | The case execution this one [corrects](#corrections) |
| `metrics` | map of `MetricInput` | no | Definitions of the metrics used in `samples` |
| `samples` | `SampleInput`[] | no | Samples, inline. More can be added later |

`TestCaseInput`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `scenario` | `NamedDefinition` | yes | Scenario name (non-empty) and definition; versioned like suites |
| `assets` | `AssetInput`[] | no | Inputs of the test case |
| `parameters` | object | no | **Flat** object of numbers, strings and booleans. Types are kept: `10` and `"10"` differ |
| `parameter_units` | map string → string | no | Units of numeric parameters, e.g. `{"bandwidth": "Mbit/s"}`. Every key must be a numeric parameter |

`AssetInput`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | Asset name (non-empty) |
| `version` | string \| null | one of the two | Version of the asset |
| `content_hash` | string \| null | one of the two | Hash of its content |

The scenario version, the set of assets and the parameters with their units
together identify the [test case](concepts.md#definitions-and-dimensions);
uploading the same combination again reuses it.

`ClientDeviceInput`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | **Stable** device name, e.g. `pixel-8-lab-1`. Trends follow it |
| `details` | object | yes | Model, OS, driver, build, ... as any JSON object. A change is marked in trends but does not split them |

`SampleInput` (inline sample):

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | UUID | yes | Client-generated sample id |
| `repetition` | integer | yes | Repetition number, `≥ 0` |
| `is_warmup` | boolean | no | Warmup sample, excluded from aggregation by default. Default `false` |
| `measurements` | map metric → value | no | One value per metric: a number, or for `boolean` metrics `true`/`false` (or `0`/`1`) |
| `series` | `SeriesInput`[] | no | Series of this sample, at most one per metric |

`SeriesInput` (inline series):

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | UUID | yes | Client-generated series id |
| `metric` | string | yes | Metric name; defined in `metrics` |
| `x_dimension` | string | yes | What x measures, e.g. `time`, `transferred`, `frame` (non-empty) |
| `x_unit` | string | yes | Unit of x; `""` for dimensionless |
| `x` | number[] | yes | x values; **never decrease** (they may repeat) |
| `y` | number[] | yes | y values; as many as `x` |

Validation of values:

- `integer` metrics need whole numbers, `boolean` metrics `true`, `false`,
  `0` or `1`; all values must be finite.
- A series has at most 1,000,000 points, `x` and `y` have the same length,
  and every value must fit a 32-bit float. Decreasing x is rejected with
  `422 series_x_decreasing`.
- Only raw, **measured** values belong in uploads. Values computed from
  them (throughput, ratios, time to a threshold) are
  [derived metrics](derived-metrics.md).

**Retry semantics.** The case execution is identical when test case,
device name, device details, `supersedes_id`, `started_at` and `ended_at`
are equal. Each inline sample and series is then checked on its own. A
retry that contains additional samples stores them and answers `201`; one
where everything existed answers `200`.

Example, with one sample inline:

```bash
curl -s -X PUT "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01/case-executions/5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "test_case": {
      "scenario": {"name": "load-model", "definition": {"camera_path": "orbit", "duration_s": 10}},
      "assets": [{"name": "engine.glb", "version": "3"}],
      "parameters": {"codec": "draco", "bandwidth": 100, "progressive": true},
      "parameter_units": {"bandwidth": "Mbit/s"}
    },
    "client_device": {
      "name": "pixel-8-lab-1",
      "details": {"model": "Pixel 8", "os": "Android 15", "browser": "Chrome 131"}
    },
    "started_at": "2026-11-20T02:00:05Z",
    "ended_at": "2026-11-20T02:01:10Z",
    "metrics": {
      "load_time": {"unit": "ms", "value_type": "float", "direction": "lower_is_better",
                    "description": "Time until the model is fully loaded"},
      "bytes_received": {"unit": "B", "value_type": "integer", "direction": "lower_is_better"},
      "fps": {"unit": "fps", "value_type": "float", "direction": "higher_is_better"}
    },
    "samples": [{
      "id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c10",
      "repetition": 0,
      "is_warmup": true,
      "measurements": {"load_time": 1840.5, "bytes_received": 52428800},
      "series": [{
        "id": "3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c10",
        "metric": "fps", "x_dimension": "time", "x_unit": "s",
        "x": [0, 0.5, 1.0, 1.5, 2.0],
        "y": [0, 24, 55, 60, 59.5]
      }]
    }]
  }'
```

```json
{"id": "5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11", "created": true}
```

| Status | Code | When |
|---|---|---|
| `201` | | Something was created |
| `200` | | Everything already existed |
| `404` | `not_found` | No such run |
| `409` | `conflict` | The case execution, a sample or a series exists with other content |
| `409` | `run_not_running` | A new case execution for a run that is not running |
| `409` | `metric_definition_conflict` | A metric exists with another unit or value type |
| `409` | `duplicate` | Two series of the same metric in one sample |
| `422` | `validation_failed` | See the validation rules above; also a `supersedes_id` that is not a case execution of the same run, test case and device |
| `422` | `series_x_decreasing` | x values of a series decrease |
| `422` | `invalid_request_body` | Schema violation, e.g. an unknown field |

### Corrections

Results are immutable (FR-6). To correct a case execution, upload a **new**
case execution (new id) with `supersedes_id` set to the one it replaces. It
must be in the same run and have the same test case and device name; it may
be added after the run was closed.

The superseded case execution is kept. It is marked with `superseded_by`,
hidden from [`GET /runs/{run_id}/case-executions`](runs.md#list-the-case-executions-of-a-run)
unless `include_superseded=true`, and left out of filters and analysis,
which only see **current** case executions.

## Add a sample

```http
PUT /api/v1/runs/{run_id}/case-executions/{case_execution_id}/samples/{sample_id}
```

Adds one sample, optionally with its series, to a case execution of the
run.

Request body `CreateSample`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `repetition` | integer | yes | Repetition number, `≥ 0` |
| `is_warmup` | boolean | no | Default `false` |
| `metrics` | map of `MetricInput` | no | Definitions of the metrics used in this request |
| `measurements` | map metric → value | no | As in [`SampleInput`](#add-a-case-execution) |
| `series` | `SeriesInput`[] | no | As in [`SeriesInput`](#add-a-case-execution), each with its own `id` |

**Retry semantics.** Identical when case execution, `repetition`,
`is_warmup` and all measurement values are equal; series are checked on
their own.

```bash
curl -s -X PUT "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01/case-executions/5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11/samples/9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "repetition": 1,
    "metrics": {
      "load_time": {"unit": "ms", "value_type": "float", "direction": "lower_is_better"},
      "bytes_received": {"unit": "B", "value_type": "integer", "direction": "lower_is_better"}
    },
    "measurements": {"load_time": 1550.5, "bytes_received": 52428800}
  }'
```

```json
{"id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11", "created": true}
```

| Status | Code | When |
|---|---|---|
| `201` / `200` | | Created / already existed |
| `404` | `not_found` | The case execution does not exist or belongs to another run |
| `409` | `conflict`, `metric_definition_conflict`, `duplicate` | As for case executions |
| `422` | `validation_failed`, `series_x_decreasing`, `invalid_request_body` | As for case executions |

## Add a series

```http
PUT /api/v1/runs/{run_id}/case-executions/{case_execution_id}/samples/{sample_id}/series/{series_id}
```

Adds one series to a sample. The series and all its points are written in
one transaction: a series is either stored completely or not at all.

Request body `CreateSeries`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `metric` | string | yes | Metric name; defined in `metrics` |
| `metrics` | map of `MetricInput` | no, but must define `metric` | Definition of the metric |
| `x_dimension` | string | yes | What x measures (non-empty) |
| `x_unit` | string | yes | Unit of x; `""` for dimensionless |
| `x` | number[] | yes | Never decreasing |
| `y` | number[] | yes | Same length as `x` |

**Retry semantics.** Identical when sample, metric, `x_dimension`,
`x_unit` and all points (as 32-bit floats) are equal.

```bash
curl -s -X PUT "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01/case-executions/5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11/samples/9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11/series/3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c11" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "metric": "fps",
    "metrics": {"fps": {"unit": "fps", "value_type": "float", "direction": "higher_is_better"}},
    "x_dimension": "time",
    "x_unit": "s",
    "x": [0, 0.5, 1.0, 1.5, 2.0],
    "y": [0, 25, 55, 60, 59.5]
  }'
```

```json
{"id": "3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c11", "created": true}
```

| Status | Code | When |
|---|---|---|
| `201` / `200` | | Created / already existed |
| `404` | `not_found` | The case execution is not in the run, or the sample not in the case execution |
| `409` | `conflict` | The series id exists with other content |
| `409` | `duplicate` | The sample already has a series of this metric |
| `409` | `metric_definition_conflict` | The metric exists with another unit or value type |
| `422` | `validation_failed` | Lengths differ, too many points, value not a finite 32-bit float, metric not defined |
| `422` | `series_x_decreasing` | x values decrease |

## Upload an attachment

```http
PUT /api/v1/runs/{run_id}/attachments/{attachment_id}?file_name=...
```

Uploads a file as the **raw request body** (not multipart, not JSON). It
belongs to the run, and optionally to one of its case executions or to a
sample of that case execution.

Query parameters:

| Parameter | Type | Required | Meaning |
|---|---|---|---|
| `file_name` | string | yes | Original file name. Used only as the download name, never as a path |
| `case_execution_id` | UUID | no | Case execution of the run the file belongs to |
| `sample_id` | UUID | no | Sample of that case execution; needs `case_execution_id` |

Headers:

| Header | Required | Meaning |
|---|---|---|
| `Content-Type` | no | Stored and sent back on download. Default `application/octet-stream` |
| `X-Content-SHA256` | no | Expected SHA-256 of the body as 64 hex digits. The upload is rejected if the body does not match |

The file is written atomically and stored under its id. Attachments can be
uploaded whatever the run's status, also after it was closed. The JSON body
limit does not apply to them.

**Retry semantics.** Identical when run, case execution, sample, file
name, content type, size and SHA-256 are equal.

```bash
curl -s -X PUT "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01/attachments/7e4a2b19-c5d8-4f36-9b0e-6a1c3d8f5e41?file_name=frames.csv&case_execution_id=5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: text/csv' \
  -H "X-Content-SHA256: $(shasum -a 256 frames.csv | cut -d' ' -f1)" \
  --data-binary @frames.csv
```

```json
{"id": "7e4a2b19-c5d8-4f36-9b0e-6a1c3d8f5e41", "created": true}
```

| Status | Code | When |
|---|---|---|
| `201` / `200` | | Created / already existed |
| `400` | `bad_request` | `X-Content-SHA256` is not 64 hex digits, or the body could not be read |
| `404` | `not_found` | The run, case execution or sample does not exist or they do not belong together |
| `409` | `conflict` | The attachment id exists with other content or metadata |
| `422` | `checksum_mismatch` | The body does not match `X-Content-SHA256` |
| `422` | `validation_failed` | Empty `file_name` or `Content-Type`; `sample_id` without `case_execution_id` |

Use `--data-binary` (not `-d`) with curl, so the file is sent unchanged.

## Close a run

```http
POST /api/v1/runs/{run_id}/close
```

Sets the final status of a running run.

Request body `CloseRun`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `status` | `completed` \| `failed` | yes | Final status. `failed` means the benchmark itself failed; the results so far are kept |
| `ended_at` | timestamp \| null | no | Measured end time. Default: now (server time) |

Closing again with the same status is a no-op (`200`, `"created": false`).
A run that already has another final status, including one that was
`aborted` for inactivity, cannot be closed
(`409 invalid_status_transition`).

```bash
curl -s -X POST "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01/close" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"status": "completed", "ended_at": "2026-11-20T02:05:00Z"}'
```

```json
{"id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01", "created": true}
```

| Status | Code | When |
|---|---|---|
| `200` | | Closed (`created: true`), or already closed with this status (`created: false`) |
| `404` | `not_found` | No such run |
| `409` | `invalid_status_transition` | The run already has another final status |

## A complete upload

A minimal harness client in Python (standard library only) that uploads a
run with one case execution, adds samples as they finish, attaches a log
and closes the run. Every id is generated once and kept, so any request
can be retried.

```python
import json, os, urllib.request, uuid

BASE = os.environ["TR_URL"] + "/api/v1"
TOKEN = os.environ["TR_TOKEN"]          # write scope

def request(method, path, body=None, raw=None, content_type="application/json"):
    data = raw if raw is not None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={
        "Authorization": f"Bearer {TOKEN}", "Content-Type": content_type})
    with urllib.request.urlopen(req) as resp:   # raises on 4xx/5xx
        return json.load(resp)

METRICS = {
    "load_time": {"unit": "ms", "value_type": "float", "direction": "lower_is_better"},
    "fps": {"unit": "fps", "value_type": "float", "direction": "higher_is_better"},
}

run_id, ce_id = uuid.uuid4(), uuid.uuid4()
request("PUT", f"/runs/{run_id}", {
    "suite": {"name": "web-viewer", "definition": {"scenarios": ["load-model"]}},
    "sut_version": {"commit": "4f2a9c1", "branch": "main"},
    "shared_environment": {"server": "render-01"},
    "triggered_by": "ci",
    "started_at": "2026-11-20T02:00:00Z",
})
request("PUT", f"/runs/{run_id}/case-executions/{ce_id}", {
    "test_case": {"scenario": {"name": "load-model", "definition": {}},
                  "parameters": {"codec": "draco"}},
    "client_device": {"name": "pixel-8-lab-1", "details": {"os": "Android 15"}},
})
for repetition in range(5):
    load_time, fps_x, fps_y = 1550.5, [0, 1, 2], [0, 55, 60]   # measured
    request("PUT", f"/runs/{run_id}/case-executions/{ce_id}/samples/{uuid.uuid4()}", {
        "repetition": repetition,
        "is_warmup": repetition == 0,
        "metrics": METRICS,
        "measurements": {"load_time": load_time},
        "series": [{"id": str(uuid.uuid4()), "metric": "fps", "x_dimension": "time",
                    "x_unit": "s", "x": fps_x, "y": fps_y}],
    })
log = b"benchmark log ...\n"
request("PUT", f"/runs/{run_id}/attachments/{uuid.uuid4()}?file_name=harness.log",
        raw=log, content_type="text/plain")
request("POST", f"/runs/{run_id}/close", {"status": "completed"})
```

In a real harness, store the generated ids before sending, so a retry after
a crash sends the same ids.
