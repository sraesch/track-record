# Conventions

Rules that hold for every endpoint of `/api/v1`.

## Authentication

Every request under `/api/v1` needs an API token in the `Authorization`
header:

```http
Authorization: Bearer trk_0VRjLeYjqEkJ44M_0Oy8mkUzuGwzVcM2vOp0EN0BwoA
```

- Tokens look like `trk_` followed by 43 base64url characters. The scheme
  name `Bearer` is case-insensitive.
- Tokens are created and revoked only in the web UI (UI port,
  `/ui/api/v1/tokens`). The secret is shown once; the server stores only its
  SHA-256.
- A missing, unknown or revoked token is answered with
  `401 unauthorized`. A token whose scope does not allow the endpoint gets
  `403 forbidden`.
- There are no user accounts. A token identifies a client (a harness, a
  script), not a person, and nothing records which token wrote what.

### Token scopes

| Scope | Allows |
|---|---|
| `read` | Every read endpoint: runs, results, definitions, annotations, derived metric validation and evaluation, analysis |
| `annotate` | `read`, plus adding and removing tags and creating, editing and deleting annotations |
| `write` | `read`, plus ingestion: creating runs, case executions, samples, series and attachments, and closing runs |

`annotate` and `write` both include `read`, but **not each other**: a
`write` token cannot add a tag after the run was created (it can set the
run's initial tags in [`PUT /runs/{run_id}`](ingestion.md#create-a-run)),
and an `annotate` token cannot upload. Give a harness that needs both two
tokens.

```json
{
  "title": "Forbidden",
  "status": 403,
  "code": "forbidden",
  "detail": "this endpoint needs a token with scope Annotate, the token has Write"
}
```

## Requests and responses

- Request and response bodies are JSON (`application/json`, UTF-8), except
  for attachment uploads and downloads, the NDJSON stream of
  [compare](analysis.md#compare) and the files of
  [export](analysis.md#export).
- A JSON body must be sent with `Content-Type: application/json`; without
  it the server answers `415`.
- **Unknown fields are rejected**: an unknown field in a request body gets
  `422 invalid_request_body`, an unknown query parameter
  `400 invalid_query`. A misspelled field never gets lost silently.
- Optional fields may be left out. Fields that are nullable (shown as
  `string | null` and the like) may also be sent as `null`; optional lists
  and maps (`tags`, `samples`, `metrics`, ...) may not. Responses always
  contain every field of their type; absent values are `null`.
- Lists in responses are plain JSON arrays, except the run list, which is
  paged (see [Paging](#paging)).
- Field names are `snake_case`. Enumerations are lower-case strings
  (`"completed"`, `"lower_is_better"`).

### Identifiers

| Kind | Type | Assigned by |
|---|---|---|
| Runs, case executions, samples, series, attachments | UUID | **The client**, when it uploads them |
| Annotations | UUID | The server |
| Definitions and dimensions: suite and scenario versions, test cases, metrics, client devices, environments, SUT versions, assets, aliases, derived metrics | 64-bit integer | The server |

Client-generated UUIDs make every upload idempotent (see below). Generate a
fresh random UUID (v4 or v7) for every result, and keep it so a retry can
reuse it. Path parameters that are not valid UUIDs or integers are answered
with `400 invalid_path`.

### Timestamps

Timestamps are RFC 3339 strings. The server accepts any offset and returns
UTC with a `Z` suffix (`2026-11-20T02:00:00Z`). They are stored with
microsecond precision; finer fractions are truncated, also when a retry is
compared with the stored content.

In query strings, encode a `+` offset as `%2B` (or use `Z`).

**Measured vs. server time.** `started_at` and `ended_at` of runs and case
executions are the times the harness measured. Trends and time filters use
them, so results can be uploaded long after the fact. `created_at`,
`updated_at` and `last_upload_at` are server time.

### Numbers

Measurements are 64-bit floats. Series points are stored as **32-bit
floats** ([ADR 0002](../adr/0002-series-storage.md)): read-back values can
differ from the uploaded ones in the last digits, and a value outside the
32-bit range is rejected. Non-finite values (`NaN`, infinities) are not valid
JSON and are rejected.

### Parameters keep their JSON type

Test case parameters are numbers, strings or booleans and keep that type:
`{"size": 10}` and `{"size": "10"}` are different test cases. The same holds
for the `param` filter (see [Run filter](analysis.md#run-filter)).

## Idempotent writes

All result uploads are `PUT` requests to a client-chosen UUID (FR-3):

| Situation | Response |
|---|---|
| The id is new | `201 Created`, `{"id": "...", "created": true}` |
| The id exists with **identical** content (a retry) | `200 OK`, `{"id": "...", "created": false}` |
| The id exists with **different** content | `409 conflict` |

So a harness can retry any request after a timeout or a lost response
without creating duplicates. Results are immutable (FR-6): content under an
existing id can never be changed. To correct a case execution, upload a new
one that [supersedes](ingestion.md#corrections) it.

Each request runs in one database transaction. A rejected request leaves
nothing behind, not even definitions (suite versions, test cases, metrics)
it would have created.

`POST /runs/{run_id}/close` is idempotent too: closing with the same status
again returns `200` with `"created": false`.

## Paging

Only [`GET /runs`](runs.md#list-runs) is paged. It returns at most `limit`
runs (1 to 500, default 50) and a `next_cursor`. Pass it as `cursor` with
the same filters to get the next page; it is `null` on the last page.
Cursors are opaque and stay valid: they encode the position, not a
snapshot.

```bash
curl -s "$TR_URL/api/v1/runs?suite=web-viewer&limit=100" -H "Authorization: Bearer $TR_TOKEN"
# ... "next_cursor": "MjAyNi0xMS0yMVQwMjowMDowMCswMDowMHwwZjhi..."
curl -s "$TR_URL/api/v1/runs?suite=web-viewer&limit=100&cursor=MjAyNi0xMS0yMVQwMjowMDowMCswMDowMHwwZjhi..." \
  -H "Authorization: Bearer $TR_TOKEN"
```

Other lists are returned whole.

## Repeated query parameters

Query parameters that take several values are repeated:
`?status=completed&status=failed&tag=nightly&tag=baseline`.

## Limits

| Limit | Value |
|---|---|
| JSON body of an ingestion request | `TR_MAX_BODY_BYTES`, default 256 MiB; larger bodies get `422 invalid_request_body` |
| Points per series | 1,000,000 |
| Attachment size | not limited by the server (a proxy or ingress in front of it may limit it) |
| Run list page | 1 to 500 runs |
| Samples one analysis may touch | 2,000,000 (`422 validation_failed`; narrow the filter) |
| Percentiles per request | 20 |
| Expression length | 4096 bytes |

## Runs that stop uploading

A run that stays `running` without any upload for `TR_STALE_RUN_TIMEOUT`
(default one hour, server time since the last upload of anything for the
run; creating it counts) is set to `aborted` (FR-2). An aborted run cannot
be closed, and no new case executions can be added to it. A harness that
pauses for longer should close the run and start a new one.

## Health and discovery

These need no token:

| Path | Response |
|---|---|
| `GET /healthz` | `200 {"status": "ok"}` while the process runs |
| `GET /readyz` | `200 {"status": "ok"}`, or `503 {"status": "database unavailable"}` |
| `GET /api/openapi.json` | The OpenAPI 3.1 document of this API |

## Versioning

The path carries the major version (`/api/v1`). Within `v1`, fields may be
added to responses and optional fields to requests; clients should ignore
response fields they do not know. Removals and incompatible changes need a
new major version. The server's release version is the `info.version` of
the OpenAPI document.
