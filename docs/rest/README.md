# REST API `/api/v1`

The machine API of track-record: the benchmark harness uploads results
through it, and scripts, notebooks and other tools read and analyse them.
It is served on the server's **machine port** (`TR_MACHINE_ADDR`, default
`0.0.0.0:8080`) and needs an API token for every request.

This documentation describes version **0.1.0**. The OpenAPI document is the
formal contract: it is served at `GET /api/openapi.json` and committed as
[`backend/openapi/machine.json`](../../backend/openapi/machine.json). Where
this text and the OpenAPI document disagree, the OpenAPI document wins;
please fix the text.

The web UI uses a second API, `/ui/api/v1`, on the UI port. It has the same
read, tag and annotation endpoints plus the operations that are reserved for
the web UI (invalidating and deleting runs, API tokens, aliases, changing
metrics and derived metrics). That API is not covered here.

## Pages

| Page | Contents |
|---|---|
| [Conventions](conventions.md) | Authentication and token scopes, formats, ids, idempotency, paging, limits, health checks |
| [Errors](errors.md) | Problem details and every error `code` |
| [Concepts](concepts.md) | The data model: runs, case executions, samples, metrics, test cases, aliases |
| [Ingestion](ingestion.md) | Uploading results: runs, case executions, samples, series, attachments |
| [Runs and results](runs.md) | Reading runs, case executions, series points and attachments |
| [Definitions](definitions.md) | Suites, scenarios, test cases, metrics, units, devices, environments, SUT versions, assets, aliases |
| [Tags and annotations](annotations.md) | Tags on runs and test cases, free-text annotations |
| [Derived metrics](derived-metrics.md) | Listing, validating and evaluating derived metrics; the expression language |
| [Analysis](analysis.md) | Run filter, aggregates, trends, comparisons, series views, export |

## Quick start

Create an API token in the web UI (**API tokens**, or `POST /ui/api/v1/tokens`
on the UI port). The secret (`trk_...`) is shown once. Then:

```bash
export TR_URL=https://track-record.example.com   # the machine port or its ingress
export TR_TOKEN=trk_...

curl -s "$TR_URL/api/v1/runs?suite=web-viewer&limit=5" \
  -H "Authorization: Bearer $TR_TOKEN"
```

To upload results you need a token with the `write` scope; the complete
upload sequence is in [Ingestion](ingestion.md#a-complete-upload).

## Endpoints

Scope is the token scope the endpoint needs (see
[Conventions](conventions.md#token-scopes)). `read` endpoints accept a
token of any scope.

### Ingestion

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `PUT` | [`/api/v1/runs/{run_id}`](ingestion.md#create-a-run) | write | Create a run |
| `PUT` | [`/api/v1/runs/{run_id}/case-executions/{case_execution_id}`](ingestion.md#add-a-case-execution) | write | Add a case execution, optionally with samples and series |
| `PUT` | [`/api/v1/runs/{run_id}/case-executions/{case_execution_id}/samples/{sample_id}`](ingestion.md#add-a-sample) | write | Add a sample |
| `PUT` | [`/api/v1/runs/{run_id}/case-executions/{case_execution_id}/samples/{sample_id}/series/{series_id}`](ingestion.md#add-a-series) | write | Add a series to a sample |
| `PUT` | [`/api/v1/runs/{run_id}/attachments/{attachment_id}`](ingestion.md#upload-an-attachment) | write | Upload an attachment |
| `POST` | [`/api/v1/runs/{run_id}/close`](ingestion.md#close-a-run) | write | Close a run as `completed` or `failed` |

### Runs and results

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `GET` | [`/api/v1/runs`](runs.md#list-runs) | read | List and filter runs |
| `GET` | [`/api/v1/runs/{run_id}`](runs.md#get-a-run) | read | Run detail |
| `GET` | [`/api/v1/runs/{run_id}/case-executions`](runs.md#list-the-case-executions-of-a-run) | read | Case executions of a run |
| `GET` | [`/api/v1/runs/{run_id}/attachments`](runs.md#list-the-attachments-of-a-run) | read | Attachments of a run |
| `GET` | [`/api/v1/case-executions/{case_execution_id}`](runs.md#get-a-case-execution) | read | Case execution with samples and measurements |
| `GET` | [`/api/v1/series/{series_id}`](runs.md#get-series-metadata) | read | Series metadata |
| `GET` | [`/api/v1/series/{series_id}/points`](runs.md#get-series-points) | read | Series points, optionally downsampled |
| `GET` | [`/api/v1/attachments/{attachment_id}`](runs.md#get-attachment-metadata) | read | Attachment metadata |
| `GET` | [`/api/v1/attachments/{attachment_id}/content`](runs.md#download-an-attachment) | read | Download an attachment |

### Definitions

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `GET` | [`/api/v1/suites`](definitions.md#suites) | read | Suites with all versions |
| `GET` | [`/api/v1/scenarios`](definitions.md#scenarios) | read | Scenarios with all versions |
| `GET` | [`/api/v1/test-cases`](definitions.md#list-test-cases) | read | Test cases |
| `GET` | [`/api/v1/test-cases/{test_case_id}`](definitions.md#get-a-test-case) | read | A test case |
| `GET` | [`/api/v1/metrics`](definitions.md#metrics) | read | Uploaded metrics |
| `GET` | [`/api/v1/units`](definitions.md#units) | read | The unit catalogue |
| `GET` | [`/api/v1/client-devices`](definitions.md#client-devices) | read | Client devices |
| `GET` | [`/api/v1/environments/{environment_id}`](definitions.md#environments) | read | A shared environment or device details |
| `GET` | [`/api/v1/sut-versions`](definitions.md#sut-versions) | read | SUT versions |
| `GET` | [`/api/v1/assets`](definitions.md#assets) | read | Assets |
| `GET` | [`/api/v1/aliases`](definitions.md#aliases) | read | Aliases with their current target |

### Tags and annotations

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `PUT` | [`/api/v1/runs/{run_id}/tags/{tag}`](annotations.md#tag-a-run) | annotate | Add a tag to a run |
| `DELETE` | [`/api/v1/runs/{run_id}/tags/{tag}`](annotations.md#remove-a-tag-from-a-run) | annotate | Remove a tag from a run |
| `PUT` | [`/api/v1/test-cases/{test_case_id}/tags/{tag}`](annotations.md#tag-a-test-case) | annotate | Add a tag to a test case |
| `DELETE` | [`/api/v1/test-cases/{test_case_id}/tags/{tag}`](annotations.md#remove-a-tag-from-a-test-case) | annotate | Remove a tag from a test case |
| `GET` | [`/api/v1/annotations`](annotations.md#list-annotations) | read | List annotations |
| `POST` | [`/api/v1/annotations`](annotations.md#create-an-annotation) | annotate | Create an annotation |
| `GET` | [`/api/v1/annotations/{annotation_id}`](annotations.md#get-an-annotation) | read | An annotation |
| `PATCH` | [`/api/v1/annotations/{annotation_id}`](annotations.md#edit-an-annotation) | annotate | Edit an annotation's text |
| `DELETE` | [`/api/v1/annotations/{annotation_id}`](annotations.md#delete-an-annotation) | annotate | Delete an annotation |

### Derived metrics

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `GET` | [`/api/v1/derived-metrics`](derived-metrics.md#list-derived-metrics) | read | Derived metrics |
| `GET` | [`/api/v1/derived-metrics/{name}`](derived-metrics.md#get-a-derived-metric) | read | A derived metric |
| `POST` | [`/api/v1/derived-metrics/validate`](derived-metrics.md#validate-an-expression) | read | Validate an expression |
| `POST` | [`/api/v1/derived-metrics/evaluate`](derived-metrics.md#evaluate-an-expression) | read | Evaluate an expression on a case execution |

### Analysis

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `POST` | [`/api/v1/analysis/aggregates`](analysis.md#aggregates) | read | Statistics per run, case and device |
| `POST` | [`/api/v1/analysis/trend`](analysis.md#trend) | read | One case, metric and device across runs |
| `POST` | [`/api/v1/analysis/compare`](analysis.md#compare) | read | Side-by-side comparison, streamed as NDJSON |
| `POST` | [`/api/v1/analysis/series/overlay`](analysis.md#series-overlay) | read | Series of many samples, optionally on a common grid |
| `POST` | [`/api/v1/analysis/series/aggregate`](analysis.md#series-aggregate) | read | Percentile bands of series per grid point |
| `POST` | [`/api/v1/analysis/export`](analysis.md#export) | read | CSV, NDJSON or Parquet export, streamed |

## What the machine API cannot do

By design (NFR-2), no API token can:

- invalidate, re-validate or delete runs,
- create or revoke API tokens,
- set or remove aliases,
- change, rename or delete metrics,
- save, rename or delete derived metrics.

These exist only on the UI port. Their paths are not routed on the machine
port at all and answer `404 not_found`.

The same tokens also authenticate the MCP server at `/mcp` on the machine
port; see [backend/README.md](../../backend/README.md#mcp).
