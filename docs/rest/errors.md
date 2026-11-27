# Errors

Every error is a problem details object
([RFC 9457](https://www.rfc-editor.org/rfc/rfc9457)) with the content type
`application/problem+json`:

```json
{
  "title": "Conflict",
  "status": 409,
  "code": "metric_definition_conflict",
  "detail": "metric load_time has unit \"ms\" and value type float, the upload sends unit \"s\" and value type float"
}
```

| Field | Type | Meaning |
|---|---|---|
| `title` | string | The HTTP reason phrase of `status` |
| `status` | integer | The HTTP status code |
| `code` | string | **Stable, machine-readable error code.** Branch on this, not on `detail` |
| `detail` | string | Human-readable explanation of this occurrence. Wording may change |

Internal errors (`500`, code `internal`) never reveal details; the server
logs them.

Errors of [compare](analysis.md#compare) that happen after the stream has
started arrive as an `error` event in the stream instead; an
[export](analysis.md#export) that fails after it has started is cut off.

## Status codes

| Status | When |
|---|---|
| `200 OK` | Success; for an ingestion `PUT`, a retry of content that already existed |
| `201 Created` | An ingestion `PUT` stored something new; an annotation was created |
| `204 No Content` | Tag added or removed, annotation deleted |
| `400 Bad Request` | Malformed request: bad JSON syntax, path or query parameter, or an option out of range |
| `401 Unauthorized` | Missing, unknown or revoked API token |
| `403 Forbidden` | The token's scope does not allow the endpoint |
| `404 Not Found` | The resource (or the endpoint) does not exist |
| `409 Conflict` | The request contradicts stored data: different content under an existing id, a run that is not running, a metric with another unit |
| `415 Unsupported Media Type` | JSON body without `Content-Type: application/json` |
| `422 Unprocessable Entity` | Well-formed but invalid: a schema violation, a failed validation rule, an invalid expression |
| `500 Internal Server Error` | A bug or an unavailable database. Safe to retry for idempotent requests |

## Error codes

### Requests

| Code | Status | Meaning |
|---|---|---|
| `unauthorized` | 401 | `missing bearer API token`, or `invalid or revoked API token` |
| `forbidden` | 403 | The token has the wrong scope |
| `not_found` | 404 | No such run, case execution, sample, series, attachment, test case, environment, annotation or derived metric; or no such endpoint |
| `invalid_path` | 400 | A path parameter is not a valid UUID or integer |
| `invalid_query` | 400 | A query parameter has the wrong type or an unknown value, or is unknown |
| `invalid_request_body` | 400, 415, 422 | 400: the body is not valid JSON. 415: `Content-Type` is not `application/json`. 422: the JSON does not match the schema (missing or unknown field, wrong type, unknown enum value) or the body is larger than the limit |
| `bad_request` | 400 | An option is out of range or inconsistent, e.g. `limit must be between 1 and 500`, `invalid cursor`, `max_points must be at least 2`, `percentiles must be at most 20 values between 0 and 100`, `give either expression or name` |
| `validation_failed` | 422 | A validation rule failed, e.g. an empty name, a parameter that is not a scalar or whose name contains a backtick, a measurement that does not fit its value type, an annotation with more than one target. `detail` names the rule |
| `internal` | 500 | Unexpected server error |

### Ingestion

| Code | Status | Meaning |
|---|---|---|
| `conflict` | 409 | The id exists with different content (FR-3, FR-6). Results never change; use a new id |
| `run_not_running` | 409 | A new case execution for a run that is `completed`, `failed` or `aborted` (only corrections may be added then) |
| `invalid_status_transition` | 409 | Closing a run that already has another final status, e.g. `completed` after `failed`, or a run that was `aborted` |
| `metric_definition_conflict` | 409 | The upload defines an existing metric with another unit or value type. Send the metric's current definition (see [`GET /metrics`](definitions.md#metrics)) |
| `series_x_decreasing` | 422 | The x values of a series decrease |
| `series_incomplete` | 422 | The stored points of a series do not match its point count (should not happen through this API) |
| `checksum_mismatch` | 422 | The attachment body does not match `X-Content-SHA256` |
| `duplicate` | 409 | A unique constraint, e.g. a second series of the same metric for one sample |
| `reference_not_found` | 422 | A referenced row does not exist |
| `check_violation` | 422 | A database check failed |
| `invalid_value` | 422 | The database rejected a value, e.g. a number out of range |
| `immutable` | 409 | An attempt to change a stored result (should not happen through this API) |

### Derived metrics and analysis

| Code | Status | Meaning |
|---|---|---|
| `invalid_expression` | 422 | The metric expression of an analysis or evaluation does not parse, refers to unknown metrics or has inconsistent units. `detail` lists the problems. Use [`validate`](derived-metrics.md#validate-an-expression) to get them with positions |
| `validation_failed` | 422 | E.g. a series where a scalar is needed (`series.fps is a series; this view needs a scalar metric, e.g. max(series.fps)`), a reference run that is not in the trend, or more than 2,000,000 samples |
| `bad_request` | 400 | E.g. `limit and cursor apply to the run list, not to analysis`, an invalid grid, `dimension parameters needs the compared parameters in \`parameters\`` |
| `derived_metric_cycle` | 422 | Derived metrics would refer to each other in a cycle |

### Seen only on the UI port

These codes belong to operations that the machine API does not offer; they
are listed so a client sharing code with the UI knows them:
`delete_not_allowed`, `derived_metric_in_use`, `metric_in_use`,
`breaks_derived_metrics`.

## Retrying

- **Ingestion** requests are idempotent. Retry them unchanged after a
  timeout, a connection error or a `5xx`: a `200` with `"created": false`
  means the first attempt had already succeeded.
- A `409 conflict` on a retry means the request was **not** identical to
  the first one (e.g. a regenerated timestamp, a reordered sample with
  another repetition). Fix the client; retrying the same request will not
  help.
- `4xx` other than `409` need a changed request.
