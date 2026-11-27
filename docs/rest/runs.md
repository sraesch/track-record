# Runs and results

Reading runs, case executions, samples, series and attachments. Every
endpoint on this page works with a token of any scope.

## List runs

```http
GET /api/v1/runs
```

Runs matching the filters, **newest measured start first** (ties by id),
paged (FR-11).

Query parameters: the [run filter](analysis.md#run-filter) plus paging.

| Parameter | Type | Meaning |
|---|---|---|
| `run_id` | UUID, repeatable | Only these runs |
| `suite` | string | Suite name |
| `sut_commit`, `sut_branch`, `sut_tag`, `sut_build_config` | string | SUT version fields, exact match |
| `started_after` | timestamp | Measured start at or after this time |
| `started_before` | timestamp | Measured start before this time |
| `started_within_days` | integer ≥ 1 | Measured start within the last N days; see [Relative selection](analysis.md#relative-selection) |
| `latest_runs` | integer ≥ 1 | Only the N matching runs with the newest measured start; `limit` and `cursor` page through them |
| `since_tag` | string | Measured start at or after that of the earliest run with this tag |
| `since_sut_tag` | string | Measured start at or after that of the earliest run whose SUT version has this tag |
| `status` | `running` \| `completed` \| `failed` \| `aborted`, repeatable | Any of these statuses |
| `triggered_by` | `nightly` \| `manual` \| `ci` | Trigger |
| `shared_environment_id` | integer | Shared environment |
| `client_device` | string | Client device name **after aliases** |
| `scenario` | string | Scenario name |
| `asset` | string | Asset name |
| `param` | `name=value`, repeatable | Parameter value after aliases; see [Run filter](analysis.md#run-filter) |
| `metric` | string | Has a measurement or series of this metric |
| `tag` | string, repeatable | Has **all** these tags |
| `validity` | `valid` \| `invalid` \| `all` | Default `valid` (FR-9) |
| `limit` | integer | Page size, 1 to 500, default 50 |
| `cursor` | string | `next_cursor` of the previous page |

Response `RunList`:

| Field | Type | Meaning |
|---|---|---|
| `runs` | `RunSummary`[] | The page |
| `next_cursor` | string \| null | Pass as `cursor` for the next page; `null` on the last page |

`RunSummary`:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | Run id |
| `suite` | object | `suite_version_id`, `name`, `version` |
| `sut_version` | `SutVersion` | `id`, `commit`, `branch`, `build_config`, `tag` |
| `shared_environment_id` | integer | See [environments](definitions.md#environments) |
| `triggered_by` | `nightly` \| `manual` \| `ci` | |
| `status` | `running` \| `completed` \| `failed` \| `aborted` | |
| `started_at` | timestamp | Measured start time |
| `ended_at` | timestamp \| null | Measured end time; for a run closed without `ended_at`, the server time of closing; `null` while running |
| `is_valid` | boolean | `false` if invalidated in the web UI |
| `invalid_reason` | string \| null | Why it was invalidated |
| `created_at` | timestamp | Upload time (server) |
| `tags` | string[] | Sorted |

```bash
curl -s "$TR_URL/api/v1/runs?suite=web-viewer&sut_branch=main&param=codec%3D%22draco%22&limit=1" \
  -H "Authorization: Bearer $TR_TOKEN"
```

```json
{
  "runs": [
    {
      "id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e02",
      "suite": {"suite_version_id": 1, "name": "web-viewer", "version": 1},
      "sut_version": {"id": 4, "commit": "8be71d0", "branch": "main", "build_config": "release", "tag": null},
      "shared_environment_id": 1,
      "triggered_by": "nightly",
      "status": "completed",
      "started_at": "2026-11-21T02:00:00Z",
      "ended_at": "2026-11-21T02:05:13Z",
      "is_valid": true,
      "invalid_reason": null,
      "created_at": "2026-11-23T19:57:13.394239Z",
      "tags": ["nightly"]
    }
  ],
  "next_cursor": "MjAyNi0xMS0yMVQwMjowMDowMCswMDowMHwwZjhiNmYyZS00ZDFhLTRjM2UtOWE1Ny0yYjhmM2MxZDllMDI"
}
```

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `limit` out of range, invalid `cursor`, a `param` without `=` or name |
| `400` | `invalid_query` | Wrong type or unknown value, e.g. `status=done` |

## Get a run

```http
GET /api/v1/runs/{run_id}
```

The run with its shared environment, annotations and attachments. Invalid
runs are returned too.

Response `RunDetail`: all fields of `RunSummary`, plus:

| Field | Type | Meaning |
|---|---|---|
| `shared_environment` | object | Details of the shared environment |
| `last_upload_at` | timestamp | Last upload of anything for this run, server time (FR-2) |
| `case_execution_count` | integer | Number of case executions |
| `annotations` | `Annotation`[] | Annotations on the run, see [Annotations](annotations.md) |
| `attachments` | `AttachmentMeta`[] | All attachments of the run |

```json
{
  "id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
  "suite": {"suite_version_id": 1, "name": "web-viewer", "version": 1},
  "sut_version": {"id": 1, "commit": "4f2a9c1", "branch": "main", "build_config": "release", "tag": null},
  "shared_environment_id": 1,
  "triggered_by": "nightly",
  "status": "completed",
  "started_at": "2026-11-20T02:00:00Z",
  "ended_at": "2026-11-20T02:05:00Z",
  "is_valid": true,
  "invalid_reason": null,
  "created_at": "2026-11-23T19:57:13.347379Z",
  "tags": ["nightly"],
  "shared_environment": {"backend_version": "2.4.1", "server": "render-01"},
  "last_upload_at": "2026-11-23T19:57:13.483718Z",
  "case_execution_count": 1,
  "annotations": [],
  "attachments": [
    {
      "id": "7e4a2b19-c5d8-4f36-9b0e-6a1c3d8f5e41",
      "run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
      "case_execution_id": "5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11",
      "sample_id": null,
      "file_name": "frames.csv",
      "content_type": "text/csv",
      "size_bytes": 23,
      "sha256": "bf8396364b947a0802f0d0d2a744937cdf09b28ab9fac55cfa85bdda209b25a5",
      "created_at": "2026-11-23T19:57:13.483718Z"
    }
  ]
}
```

`404 not_found` if the run does not exist.

## List the case executions of a run

```http
GET /api/v1/runs/{run_id}/case-executions
```

| Parameter | Type | Meaning |
|---|---|---|
| `include_superseded` | boolean | Also return case executions replaced by a [correction](ingestion.md#corrections). Default `false` |

Sorted by scenario, test case, device name and upload time. Response:
`CaseExecutionSummary`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | Case execution id |
| `run_id` | UUID | Its run |
| `test_case` | `TestCaseSummary` | See [test cases](definitions.md#list-test-cases) |
| `client_device` | object | `id`, `name` (as uploaded), `resolved_name` (after aliases; trends and comparisons use this) |
| `device_environment_id` | integer | The device details, see [environments](definitions.md#environments) |
| `supersedes_id` | UUID \| null | The case execution this one corrects |
| `superseded_by` | UUID \| null | Set if a correction replaced this one |
| `started_at`, `ended_at` | timestamp \| null | Measured times |
| `created_at` | timestamp | Upload time |
| `sample_count` | integer | Number of samples |

```json
[
  {
    "id": "5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11",
    "run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
    "test_case": {
      "id": 1,
      "scenario": "load-model",
      "scenario_version": 1,
      "scenario_version_id": 1,
      "parameters": {"bandwidth": 100, "codec": "draco", "progressive": true},
      "resolved_parameters": {"bandwidth": 100, "codec": "draco", "progressive": true},
      "parameter_units": {"bandwidth": "Mbit/s"},
      "assets": [{"id": 1, "name": "engine.glb", "version": "3", "content_hash": null}],
      "tags": []
    },
    "client_device": {"id": 1, "name": "pixel-8-lab-1", "resolved_name": "pixel-8-lab-1"},
    "device_environment_id": 5,
    "supersedes_id": null,
    "superseded_by": null,
    "started_at": "2026-11-20T02:00:05Z",
    "ended_at": "2026-11-20T02:01:10Z",
    "created_at": "2026-11-23T19:57:13.401757Z",
    "sample_count": 3
  }
]
```

`404 not_found` if the run does not exist.

## Get a case execution

```http
GET /api/v1/case-executions/{case_execution_id}
```

A case execution with its device details and all samples, measurements and
series metadata (not the points). Superseded case executions can be read
too.

Response `CaseExecutionDetail`: all fields of `CaseExecutionSummary`, plus:

| Field | Type | Meaning |
|---|---|---|
| `device_details` | object | Details of the client device for this execution |
| `samples` | `SampleDetail`[] | By repetition |

`SampleDetail`:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | Sample id |
| `repetition` | integer | |
| `is_warmup` | boolean | |
| `created_at` | timestamp | Upload time |
| `measurements` | `MeasurementOut`[] | By metric name: `metric`, `metric_id`, `unit`, `value` |
| `series` | `SeriesMeta`[] | By metric name, see [below](#get-series-metadata) |

`unit` is the unit the metric has **now**. Values are never converted: if a
metric's unit was changed in the web UI, stored values are read in the new
unit. Boolean measurements are returned as `0` or `1`.

```json
{
  "id": "5c2d7a10-8e3f-4b6a-b1c9-7d4e2f0a6b11",
  "run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
  "test_case": {"id": 1, "scenario": "load-model", "...": "..."},
  "client_device": {"id": 1, "name": "pixel-8-lab-1", "resolved_name": "pixel-8-lab-1"},
  "device_environment_id": 5,
  "supersedes_id": null,
  "superseded_by": null,
  "started_at": "2026-11-20T02:00:05Z",
  "ended_at": "2026-11-20T02:01:10Z",
  "created_at": "2026-11-23T19:57:13.401757Z",
  "sample_count": 3,
  "device_details": {"browser": "Chrome 131", "model": "Pixel 8", "os": "Android 15"},
  "samples": [
    {
      "id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c10",
      "repetition": 0,
      "is_warmup": true,
      "created_at": "2026-11-23T19:57:13.401757Z",
      "measurements": [
        {"metric": "bytes_received", "metric_id": 1, "unit": "B", "value": 52428800.0},
        {"metric": "load_time", "metric_id": 3, "unit": "ms", "value": 1840.5}
      ],
      "series": [
        {
          "id": "3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c10",
          "sample_id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c10",
          "metric": "fps",
          "metric_id": 2,
          "unit": "fps",
          "x_dimension": "time",
          "x_unit": "s",
          "point_count": 5
        }
      ]
    }
  ]
}
```

(`test_case` shortened; it has the same fields as in the list.)

`404 not_found` if the case execution does not exist.

## Get series metadata

```http
GET /api/v1/series/{series_id}
```

Response `SeriesMeta`:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | Series id |
| `sample_id` | UUID | Its sample |
| `metric`, `metric_id` | string, integer | The metric |
| `unit` | string | Unit of y: the unit the metric has now |
| `x_dimension` | string | What x measures |
| `x_unit` | string | Unit of x |
| `point_count` | integer | Number of points |

```json
{
  "id": "3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c11",
  "sample_id": "9a1e4c55-2b7d-4f80-a3c6-1e5d8b0f2c11",
  "metric": "fps",
  "metric_id": 2,
  "unit": "fps",
  "x_dimension": "time",
  "x_unit": "s",
  "point_count": 5
}
```

`404 not_found` if the series does not exist.

## Get series points

```http
GET /api/v1/series/{series_id}/points
```

| Parameter | Type | Meaning |
|---|---|---|
| `max_points` | integer | Downsample for display to at most this many points, keeping the minimum and maximum of each bucket (FR-19). At least 2. Without it, all points |

Response `SeriesPoints`: `id`, and `x` and `y` as arrays of equal length.
Values are 32-bit floats (see [Numbers](conventions.md#numbers)).

```bash
curl -s "$TR_URL/api/v1/series/3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c11/points" \
  -H "Authorization: Bearer $TR_TOKEN"
```

```json
{
  "id": "3b7f1d9c-6a2e-4c58-8f0b-1e5d8b0f2c11",
  "x": [0.0, 0.5, 1.0, 1.5, 2.0],
  "y": [0.0, 25.0, 55.0, 60.0, 59.5]
}
```

Downsampling is meant for charts; use the full points (or
[derived metrics](derived-metrics.md)) for computations.

| Status | Code | When |
|---|---|---|
| `400` | `bad_request` | `max_points` below 2 |
| `404` | `not_found` | No such series |

## List the attachments of a run

```http
GET /api/v1/runs/{run_id}/attachments
```

All attachments of the run, including those of its case executions and
samples, by upload time. Response: `AttachmentMeta`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | Attachment id |
| `run_id` | UUID | Its run |
| `case_execution_id` | UUID \| null | Its case execution, if any |
| `sample_id` | UUID \| null | Its sample, if any |
| `file_name` | string | Original file name |
| `content_type` | string | `Content-Type` sent with the upload |
| `size_bytes` | integer | Size |
| `sha256` | string | SHA-256 of the content, hex |
| `created_at` | timestamp | Upload time |

A run that does not exist has no attachments: the response is `[]`.

## Get attachment metadata

```http
GET /api/v1/attachments/{attachment_id}
```

Response: one `AttachmentMeta`. `404 not_found` if it does not exist.

## Download an attachment

```http
GET /api/v1/attachments/{attachment_id}/content
```

The file as it was uploaded, streamed. Response headers:

| Header | Value |
|---|---|
| `Content-Type` | The content type of the upload |
| `Content-Length` | The size |
| `Content-Disposition` | `attachment`, with the original file name (ASCII fallback and UTF-8 `filename*`) |
| `X-Content-Type-Options` | `nosniff` |
| `Content-Security-Policy` | `sandbox` |

```bash
curl -s -OJ "$TR_URL/api/v1/attachments/7e4a2b19-c5d8-4f36-9b0e-6a1c3d8f5e41/content" \
  -H "Authorization: Bearer $TR_TOKEN"
```

`404 not_found` if the attachment does not exist.
