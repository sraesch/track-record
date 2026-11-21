# Benchmark Results Store – Requirements

## 1. Context & Scope

Complex solutions evolve over weeks, months or years. Benchmark results are a key
input for design decisions and for tracking performance across that lifecycle.
This system stores, manages and supports the analysis of benchmark results.

- **Users:** a small team sharing one instance.
- **Scope:** one instance per solution under test (no multi-project support).
- **Non-goals:**
  - Executing benchmarks (a separate harness does that and uploads results).
  - Automatic regression detection or alerting. Regressions are identified by
    people through analysis (comparisons, trends) that the system supports.
  - User management and authentication (handled by an ingress/auth proxy).
  - General-purpose time-series storage.

## 2. Domain Model

| Entity | Description |
|---|---|
| **SUT Version** | Exact version of the solution under test: commit, branch, build configuration, optional tag. |
| **Suite** | Named, versioned set of scenarios. |
| **Run** | One execution of a suite (e.g. nightly). References the SUT version and the **shared environment** (e.g. server/backend). Has trigger (nightly/manual/CI), start/end time, status, validity, tags, annotations. |
| **Environment** | Hardware, OS, drivers, runtime and configuration. Two roles: the *shared environment* (per run) and the *client device* (per case execution). Shared environments are identified by a fingerprint so identical ones are deduplicated. |
| **Client Device** | Identified by a **stable name** sent by the harness (e.g. `pixel-8-lab-1`). Its details (model, OS, driver, build) are recorded with every case execution. Detail changes are marked in trends but do **not** break them. |
| **Asset** | Input resource (dataset, model, …), identified by name + version/content hash. |
| **Scenario** | Test case definition. Any change to the definition creates a new scenario version, which is treated as a **different case** (the trend starts over; old and new are never mixed). |
| **Parameters** | Free-form key/value pairs that keep their JSON type (number, string, bool) for filtering, sorting and use as an axis. |
| **Case** | Scenario version + assets + parameter values. The unit that is compared across runs. |
| **Case Execution** | One case executed within a run on one client device. Groups the samples. |
| **Sample** | One repetition of a case execution, with a warmup flag. All samples are stored. |
| **Metric** | Definition: name, unit, value type, direction (higher/lower is better), description. |
| **Measurement** | Scalar value of a metric for one sample. |
| **Series** | Ordered (x, y) points of a metric for one sample, with an x-dimension and unit (e.g. time [s], transferred [bytes]). Up to ~10k points. |
| **Derived Metric** | Versioned expression over metrics and series, evaluated at query time (see FR-17). |
| **Alias** | Globally stored, versioned mapping that renames or merges dimension values (e.g. two device names → one device). Applies to UI, API and MCP alike. |
| **Attachment** | File (screenshot, log, trace, profile) linked to a run, case execution or sample. |
| **Annotation / Tag** | Free-text note or label on a run, case or point in time (e.g. "switched to new compression"). |

**Comparison key for trends:** Case + client device name (after aliases are applied).

## 3. Functional Requirements

### Ingestion
- **FR-1** Results are uploaded **incrementally**. A run is created at start, case
  executions/samples/series are added as they finish, and the run is closed at the
  end. Partial results survive harness crashes.
- **FR-2** Run status lifecycle: `running → completed | failed | aborted`. Runs that
  stay in `running` longer than a configurable timeout are marked `aborted`.
- **FR-3** Uploads are idempotent (client-generated IDs), so retries don't create
  duplicates.
- **FR-4** Uploads are validated against a documented schema (OpenAPI / JSON Schema).
- **FR-5** Attachments are uploaded alongside results and stored as files on a
  shared volume (see [ADR 0003](adr/0003-attachment-storage.md)).

### Data integrity, versioning & lifecycle
- **FR-6** Uploaded results are immutable. Corrections create new revisions.
- **FR-7** Suites, scenarios, metric definitions, derived metrics and aliases are
  versioned. Results reference the exact definition versions used.
- **FR-8** All raw data (samples, series, attachments) is kept indefinitely. Nothing
  is deleted automatically.
- **FR-9** Runs can be **marked invalid** with a reason. Invalid runs stay stored,
  are excluded from analysis by default, can be shown on request, and can be
  marked valid again.
- **FR-10** Runs can be **hard-deleted** (e.g. test uploads), including their series
  and attachments. Deletion is immediate and requires typing the run ID to confirm.
  Because the system has no user identity (see NFR-2), deletion is available to
  everyone with UI access. Deleted data can only be recovered from backup (NFR-8).

### Querying & analysis
- **FR-11** Filter by SUT version, time range, shared environment, client device,
  asset, scenario, parameters, metric, tags and validity.
- **FR-12** Aggregate across samples: mean, median, percentiles, stddev, confidence
  intervals, with warmup samples excluded by default.
- **FR-13** **Value transforms:** unit conversion, normalization relative to a
  user-selected reference run/case, log scale.
- **FR-14** **Relabel / join:** apply global aliases to rename or merge dimension
  values.
- **FR-15** **Parameters as axes:** use any parameter as x-axis or grouping
  dimension (e.g. load time vs. model size).
- **FR-16** Compare runs side by side (run A vs. run B, or vs. a user-selected
  reference run), showing absolute and relative differences.
- **FR-17** Derived metrics are defined in the system as versioned expressions and
  evaluated at query time, so they also apply to historical runs. Supported:
  - **Scalar arithmetic** over metrics of the same sample (e.g. throughput =
    bytes / time).
  - **Series reductions** (series → scalar): min, max, mean, area under curve,
    time-to-threshold (e.g. time until 90 % loaded).
  - **Series transforms** (series → series): derivative/rate, cumulative sum,
    moving average.
  - Cross-run expressions are out of scope; FR-13 normalization covers comparison
    against a reference.
- **FR-17a** Derived metrics are written in a **small custom DSL**, e.g.
  `bytes / time`, `max(series.fps)`, `time_to(series.loaded, 0.9)`,
  `moving_avg(series.fps, 1.0)`. The DSL:
  - has a fixed set of operators and functions (no loops, no I/O), so it is safe
    to evaluate on the server;
  - is validated on save (syntax, referenced metrics exist, unit consistency);
  - is the same in UI, REST API and MCP.
- **FR-18** Trend view: a case/metric/device across runs over time or over SUT
  versions, with annotations, device detail changes and environment changes marked.
- **FR-19** Series operations: overlay series from different samples/runs, resample
  and align to a common x-grid, aggregate across samples (e.g. median series with a
  percentile band), downsample for display.
- **FR-20** Export query results (CSV, Parquet, JSON) for external analysis.

## 4. Interfaces

- **REST API** (OpenAPI-specified): ingestion, retrieval, querying, comparison,
  attachments, annotations, aliases.
- **MCP server:** read + annotate.
  - Read: list/search runs, get run details, compare runs, get trends, query
    aggregates and derived metrics, list scenarios/metrics/devices.
  - Write: add/edit tags and annotations only. No uploads, invalidation or deletion.
- **Web UI:** run list, run detail (incl. attachments), run comparison, trend view,
  series overlay, faceted filtering, annotation editing, management of definitions,
  derived metrics and aliases, invalidation/deletion of runs.

## 5. Non-Functional Requirements

- **NFR-1 Deployment:** Kubernetes (Helm chart). Stateless backend that can scale
  horizontally. A shared `ReadWriteMany` volume for attachments.
- **NFR-2 Access:**
  - **Web UI:** authentication is done by an ingress/auth proxy in front of the
    system. The app has **no user identity**: everyone with access has the same
    rights, and annotations have no author.
  - **Machine clients (harness, MCP):** a separate ingress path for `/api` and
    `/mcp` bypasses the proxy. The app checks **API tokens** itself.
  - Token scopes: `read`, `annotate` (read + tags/annotations, used by MCP) and
    `write` (ingestion). Deletion and invalidation are **not** possible with any
    token; they are only available in the web UI.
  - Tokens are created and revoked in the web UI and stored only as hashes.
- **NFR-3 Volume:**
  - 1–5 runs per day.
  - More than 10k series per run, up to ~10k points each, so up to ~100M points per
    run and roughly 10¹⁰–10¹¹ points per year.
  - With indefinite retention, plan for **terabytes over the system's lifetime**.
- **NFR-4 Series storage:** to avoid premature optimization, series points are
  initially stored in PostgreSQL, one row per point (see
  [ADR 0002](adr/0002-series-storage.md)). All access to series data goes through
  one storage component, so it can move to compressed blocks (e.g. delta + zstd,
  or Parquet in object storage) if the real volume requires it.
- **NFR-5 Query cost:** derived metrics are computed at query time over many series
  (a full run comparison touches 10⁸+ points). Results of series reductions may be
  **cached** (never treated as the source of truth) if needed to meet NFR-6.
- **NFR-6 Performance** (relaxed targets, measured on the reference deployment):

  | Operation | Target |
  |---|---|
  | Trend query over 1 year (one case/metric/device) | < 3 s |
  | Full run-vs-run comparison | < 30 s, with progress indicator |
  | Ingest one case execution with all samples/series | < 10 s |

- **NFR-7 Long operations:** operations that may take longer than a few seconds
  (comparisons, exports) show progress in the UI. In the API they either stream
  results or run as jobs that can be polled.
- **NFR-8 Backup/restore** of the database and the attachment volume. Because hard delete
  is immediate (FR-10), backups are the only way to recover deleted runs. Backup
  retention must be documented.

## 6. Open Points

None. All open points have been decided. Remaining choices (storage format for
series blocks, DSL grammar, cache strategy) are design decisions, not
requirements.
