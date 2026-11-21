# Track Record

A store for benchmark results. It keeps every result of a solution under test
over its whole lifecycle and helps the team analyze them: run comparisons,
trends over time and versions, series overlays and derived metrics.

> **Status:** early stage. The requirements are fixed; the design and
> implementation are not started yet. See [docs/requirements.md](docs/requirements.md).

## What it does

- **Incremental ingestion** from an external benchmark harness: a run is opened,
  case executions, samples and series are added as they finish, and the run is
  closed. Partial results survive harness crashes. Uploads are idempotent and
  schema-validated.
- **Immutable, versioned data**: results never change after upload (corrections
  create revisions). Suites, scenarios, metrics, derived metrics and aliases are
  versioned, and results reference the exact versions used. Raw data is kept
  indefinitely.
- **Analysis**: filtering on any dimension, sample aggregation (mean, median,
  percentiles, stddev, confidence intervals), unit conversion and normalization,
  parameters as axes, run-vs-run comparison, trend views with annotations, and
  series operations (overlay, resample, aggregate, downsample).
- **Derived metrics** written in a small, safe DSL and evaluated at query time,
  so they also apply to historical runs:
  `bytes / time`, `max(series.fps)`, `time_to(series.loaded, 0.9)`,
  `moving_avg(series.fps, 1.0)`.
- **Export** of query results as CSV, Parquet or JSON.

### Non-goals

- Running benchmarks (a separate harness does that).
- Automatic regression detection or alerting.
- User management and authentication (an ingress/auth proxy handles it).
- General-purpose time-series storage.
- Multiple projects per instance (one instance per solution under test).

## Core concepts

| Concept | In short |
|---|---|
| **Run** | One execution of a suite against a SUT version on a shared environment. Status `running → completed \| failed \| aborted`. |
| **Case** | Scenario version + assets + parameter values. The unit compared across runs. |
| **Case execution** | One case run on one client device within a run. Holds the samples. |
| **Sample** | One repetition (may be warmup). Holds scalar measurements and series. |
| **Series** | Ordered (x, y) points of a metric for one sample, up to ~10k points. |
| **Client device** | Identified by a stable name. Detail changes are marked in trends but do not break them. |
| **Alias** | Global, versioned mapping that renames or merges dimension values. |

Trends are keyed by **case + client device name** (after aliases are applied).
A changed scenario definition is a new scenario version, and so a new case: its
trend starts over.

See [docs/requirements.md](docs/requirements.md) for the full domain model.

## Interfaces

- **REST API** (OpenAPI): ingestion, retrieval, querying, comparison,
  attachments, annotations, aliases.
- **MCP server**: read access plus editing tags and annotations.
- **Web UI**: run list and details, comparison, trends, series overlay, faceted
  filtering, definitions and alias management, run invalidation and deletion.

## Access model

- The **web UI** sits behind an ingress/auth proxy. The app has no user identity:
  everyone with access has the same rights.
- **Machine clients** (harness, MCP) use `/api` and `/mcp` on a separate ingress
  path and authenticate with API tokens checked by the app. Scopes: `read`,
  `annotate`, `write`. Tokens are managed in the web UI and stored as hashes.
- **Invalidation and hard deletion of runs are only possible in the web UI**,
  never with a token.

## Deployment

- Kubernetes via a Helm chart, with a stateless, horizontally scalable backend.
- PostgreSQL (or similar) for metadata; S3-compatible object storage for
  attachments and compressed series blocks.
- Backups of both the database and object storage are required. They are the
  only way to recover a hard-deleted run.

## Expected volume

1–5 runs per day, 10k+ series per run with up to ~10k points each. That is up to
~100M points per run and terabytes over the system's lifetime, so series are
stored as compressed blocks, never as one row per point.

| Operation | Target |
|---|---|
| Trend query over 1 year (one case/metric/device) | < 3 s |
| Full run-vs-run comparison | < 30 s, with progress |
| Ingest one case execution with all samples/series | < 10 s |

## License

[MIT](LICENSE)
