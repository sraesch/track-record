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
  - Charting of other data. Charts (FR-22) show the results stored here and
    nothing else.

## 2. Domain Model

| Entity | Description |
|---|---|
| **SUT Version** | Exact version of the solution under test: commit, branch, build configuration, optional tag. |
| **Suite** | Named, versioned set of scenarios. |
| **Run** | One execution of a suite (e.g. nightly). References the SUT version and the **shared environment** (e.g. server/backend). Has trigger (nightly/manual/CI), start/end time as measured by the harness, status, validity, tags, annotations. |
| **Environment** | Hardware, OS, drivers, runtime and configuration. Two roles: the *shared environment* (per run) and the *client device* (per case execution). Shared environments are identified by a fingerprint so identical ones are deduplicated. |
| **Client Device** | Identified by a **stable name** sent by the harness (e.g. `pixel-8-lab-1`). Its details (model, OS, driver, build) are recorded with every case execution. Detail changes are marked in trends but do **not** break them. |
| **Asset** | Input resource (dataset, model, …), identified by name + version/content hash. |
| **Scenario** | Test case definition. Any change to the definition creates a new scenario version, which is treated as a **different case** (the trend starts over; old and new are never mixed). |
| **Parameters** | Free-form key/value pairs that keep their JSON type (number, string, bool) for filtering, sorting and use as an axis. A numeric parameter can carry a unit (FR-13a). |
| **Case** | Scenario version + assets + parameter values. The unit that is compared across runs. |
| **Case Execution** | One case executed within a run on one client device. Groups the samples. |
| **Sample** | One repetition of a case execution, with a warmup flag. All samples are stored. |
| **Metric** | Definition: name, unit (FR-13a), value type, direction (higher/lower is better), description. It is not versioned: it has one definition, which can be changed in the web UI (FR-7). |
| **Measurement** | Scalar value of a metric for one sample. Only raw, measured values are uploaded; metrics computed from them are derived metrics (FR-17). |
| **Series** | Ordered (x, y) points of a metric for one sample, with an x-dimension and unit (e.g. time [s], transferred [bytes]). One series per metric per sample. Only raw, measured series are uploaded; a curve over another x-dimension is a derived metric (FR-17). x never decreases but may repeat. Up to ~10k points. |
| **Derived Metric** | Expression over metrics and series, evaluated at query time (see FR-17). It is not versioned: it has one definition, which can be changed. |
| **Alias** | Globally stored, versioned mapping that renames or merges dimension values (e.g. two device names → one device). Dimension values include the values of a case parameter, which keep their JSON type (e.g. a renamed pipeline keeps its trend). Applies to UI, API and MCP alike. |
| **Attachment** | File (screenshot, log, trace, profile) linked to a run, case execution or sample. |
| **Annotation / Tag** | Free-text note or label on a run, case or point in time (e.g. "switched to new compression"). |
| **Frame** | The unit of data in a chart (FR-22): an ordered list of points with named values and their units. One series of one sample is a frame; a scalar of one sample is a frame with one point. Frames and points carry the attributes of what they come from: run, SUT version, shared environment, case, client device and sample. |
| **Group** | An ordered list of frames, or of groups, in the graph of a chart. The attribute values that its members share define it (FR-22). |
| **Chart** | A saved analysis: a graph of nodes that prepares frames and ends in one visualization (FR-22 to FR-24). It can have variables (FR-25) and keeps a history of its definitions (FR-26). |
| **Dashboard** | An arrangement of charts with shared variables (FR-25). |
| **Built-in chart** | A chart that ships with the system and cannot be changed: comparison, trend and series view (FR-27). |

**Comparison key for trends:** Case + client device name (after aliases are applied).

## 3. Functional Requirements

### Ingestion
- **FR-1** Results are uploaded **incrementally**. A run is created at start, case
  executions/samples/series are added as they finish, and the run is closed at the
  end. Partial results survive harness crashes.
- **FR-2** Run status lifecycle: `running → completed | failed | aborted`. A
  `running` run that receives no upload for longer than a configurable timeout is
  marked `aborted`. The timeout counts server time since the last upload (creating
  the run counts as one), never the measured start time, so a run that is uploaded
  after it was measured, or imported later, is not aborted. Trends and time filters
  use the measured time.
- **FR-3** Uploads are idempotent (client-generated IDs), so retries don't create
  duplicates.
- **FR-4** Uploads are validated against a documented schema (OpenAPI / JSON Schema).
- **FR-5** Attachments are uploaded alongside results and stored as files on a
  shared volume (see [ADR 0003](adr/0003-attachment-storage.md)).

### Data integrity, versioning & lifecycle
- **FR-6** Uploaded results are immutable. Corrections create new revisions.
- **FR-7** Suites, scenarios and aliases are versioned. Results reference the
  exact definition versions used. Derived metrics are not versioned (FR-17):
  no result refers to them. Metrics are not versioned either
  ([ADR 0018](adr/0018-metrics-without-versions.md)): a metric has one
  definition, which can be changed in the web UI, and a change applies to
  all its results, historical ones included. Stored values are not
  converted; with another unit they are read in that unit. Uploads create
  metrics but never change them: an upload with another unit or value type
  than the metric has is rejected. A metric can be renamed (the derived
  metrics and charts that use it are changed to the new name) and, while it
  has no results and no derived metric or chart uses it, deleted.
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
  asset, scenario, parameters, metric, tags and validity. The time range can
  be relative, so that a saved view follows new runs (FR-21): the last N days,
  the latest N runs, or the runs since a marked run (a run with a given tag
  or SUT tag).
- **FR-12** Aggregate across samples: mean, median, percentiles, stddev, confidence
  intervals, with warmup samples excluded by default.
- **FR-12a** A derived metric can have **no value** (e.g. time-to-threshold when the
  threshold is never reached). Uploaded measurements always have a value. No value
  is shown as such and is not the same as missing data. Aggregates use the samples
  that have a value and state how many had none. A difference against no value is
  shown as not computable. The built-in charts (FR-27) always show this. An own
  chart (FR-22) gets the counts with every aggregate and decides what it shows.
- **FR-13** **Value transforms:** unit conversion, normalization relative to a
  user-selected reference run/case, log scale.
- **FR-13a** **Units:** every metric, every series x-dimension and, where it
  applies, every numeric parameter has a defined unit. The system provides a
  broad catalogue of predefined units with their conversions (e.g. time, data
  size, data rate, frequency, ratio/percent, count). Units outside the catalogue
  are allowed but are not converted.
- **FR-14** **Relabel / join:** apply global aliases to rename or merge dimension
  values.
- **FR-15** **Parameters as axes:** use any parameter as x-axis or grouping
  dimension (e.g. load time vs. model size).
- **FR-16** Compare side by side along a chosen **dimension**: the run (run A vs.
  run B), the client device, or one or more case parameters (e.g. two variants of
  a pipeline). Two or more values of the dimension are shown, one of them as the
  user-selected reference, with absolute and relative differences (FR-13),
  aggregated across samples (FR-12).
  - Results are matched on everything that is not compared: scenario version,
    assets, client device and the remaining parameters. The user can take
    parameters out of this matching key (e.g. settings that depend on the compared
    parameter); their values are then shown with each compared value.
  - A comparison covers one run or a filtered set of runs (FR-11). Within one run,
    a cell shows the aggregate across samples with its spread (FR-12). If several
    runs have results for the same cell, it shows one value per run in time order
    and links to the trend (FR-18); samples of different runs are never pooled.
    Differences to the reference then use the latest run by measured start time.
  - Compared columns that come from different SUT versions or shared environments
    are marked.
  - The comparison is a built-in chart (FR-27), and these rules bind it. Own
    charts (FR-22) can match and merge results in other ways.
- **FR-17** Derived metrics are defined in the system as expressions and
  evaluated at query time, so they also apply to historical runs. A derived
  metric has one definition, which can be changed; there are no versions. A
  change applies to every analysis from then on, historical runs included.
  A derived metric can be renamed; the expressions of the derived metrics
  and charts that use it are changed to the new name. It can be deleted when
  no other derived metric and no chart uses it. Supported:
  - **Scalar arithmetic** over metrics of the same sample (e.g. throughput =
    bytes / time), including log10 and clamping to a range.
  - **Series reductions** (series → scalar): min, max, mean, area under curve,
    time-to-threshold (e.g. time until 90 % loaded). A reduction can be limited
    to an x-range, and time-to-threshold can start at an x offset; bounds and
    offsets are constants or scalars of the same sample. A reduction without a
    result (threshold never reached) yields no value (FR-12a).
  - **Series transforms** (series → series): derivative/rate, cumulative sum,
    moving average, log10 of x, and re-keying one series by another series of the
    same sample (e.g. quality over time and bytes received over time → quality
    over bytes).
  - **Pointwise math** on series of the same sample: the scalar operators and
    functions applied point by point, between a series and a scalar or between
    two series with the same x values (e.g. quality from two raw scores).
  - **Interpolation** between points is linear or step (a value is held until the
    next point and beyond the last one), chosen in the expression.
  - **Composition:** a derived metric can use other derived metrics, scalar or
    series, as long as there is no cycle. A change to a derived metric takes
    effect in the metrics built on it. A change that would make one of them
    invalid is rejected.
  - Cross-run expressions are out of scope for the DSL: a derived metric works
    within one sample. Results of different runs are related in the graph of a
    chart (FR-22); FR-13 normalization covers comparison against a reference.
- **FR-17a** Derived metrics are written in a **small custom DSL**, e.g.
  `bytes / time`, `max(series.fps)`, `time_to(series.loaded, 0.9)`,
  `moving_avg(series.fps, 1.0)`. The DSL:
  - has a fixed set of operators and functions (no loops, no I/O), so it is safe
    to evaluate on the server;
  - is validated on save (syntax, referenced metrics exist, no cycles between
    derived metrics, unit consistency, and the derived metrics built on the
    saved one stay valid);
  - is the same in UI, REST API and MCP.
- **FR-18** Trend view: a case/metric/device across runs over time or over SUT
  versions, with annotations, device detail changes and environment changes marked.
  It is a built-in chart (FR-27).
- **FR-19** Series operations: overlay series from different samples/runs, resample
  and align to a common x-grid, aggregate across samples (e.g. median series with a
  percentile band), downsample for display. The series view is a built-in chart
  (FR-27); the operations are also nodes of own charts (FR-22).
- **FR-20** Export the data of a chart (CSV, Parquet, JSON) for external analysis.
- **FR-21** Every run, case execution, chart and dashboard has a URL that
  reproduces it, including the values of its variables (FR-25), its filters among
  them. Views are live: they reflect later runs, alias changes and invalidations,
  and they show the current definition of a chart.

### Charts and dashboards

The design is in [ADR 0022](adr/0022-chart-and-dashboard-designer.md).

- **FR-22** **Charts.** An analysis is built as a chart: a graph of nodes that
  prepares results and ends in one visualization (FR-24).
  - A graph starts from one or more **sources**. A source is a run filter (FR-11)
    and a metric expression (FR-17a). It yields one **frame** per sample: the
    points of a series, or one point for a scalar. Aliases are applied. Invalid
    runs and warmup samples are left out unless the source asks for them;
    superseded results are never used.
  - Frames and points carry the **attributes** of what they come from: run, SUT
    version, shared environment, case (scenario version, assets, parameters),
    client device and sample. A point that results from a reduction keeps the
    attributes of the frame it was reduced from, so any attribute can be an axis
    (FR-15).
  - The **nodes** are a fixed set of operations:
    - filter frames or points by conditions;
    - derive an attribute from other attributes with an expression;
    - group frames by attributes (groups can be nested) and flatten a group level;
    - sort, and select the latest runs within each group;
    - resample series onto a common x-grid, with linear or step interpolation;
    - match the frames of two branches that agree on a key;
    - reduce every frame to one point, so that the frames of a group become one
      frame;
    - concatenate the points of the frames of a group;
    - combine the frames of a group point by point: with a function or, after a
      match, with expressions (e.g. the difference or ratio to a reference).
  - Reduce and combine offer fixed lists of functions: mean, median, percentiles,
    confidence interval, minimum, maximum, sum, count, standard deviation and
    geometric mean. The expressions of derived attributes and of combine are
    written in the syntax of the DSL (FR-17a), with its unit rules. A graph has no
    loops and no other code.
  - Series are combined point by point only on equal x values. Resampling is
    always an explicit node.
  - **Own charts are free.** The rules that the built-in charts keep (FR-27) do
    not bind them, and a chart that breaks them is not marked.
- **FR-23** **Editor.** Charts are built in the web UI in a node graph editor. It
  validates the graph while it is edited and shows what each node produces on a
  limited part of the data, marked as partial.
- **FR-24** **Visualizations:** line (with bands, and with the marks of FR-18),
  scatter, bar, box plot, heatmap, single value and table. The columns and
  attributes of the data and the group levels are assigned to the roles of a
  visualization (axes, color, panel). Unit conversion and log scale (FR-13) are
  options of an axis.
- **FR-25** **Variables and dashboards.** A chart can have variables (e.g. a run
  filter, a metric, a device) that its nodes use. A dashboard arranges charts and
  passes shared variables to them.
- **FR-26** **Saving and history.** Charts and dashboards are saved in the web UI
  and shared by everyone with access. Every save keeps the earlier definitions,
  and an earlier one can be restored. A chart that a dashboard uses cannot be
  deleted.
- **FR-27** **Built-in charts.** The comparison (FR-16), the trend (FR-18) and the
  series view (FR-19) ship as charts. They cannot be changed, only duplicated as
  own charts. They keep these rules:
  - Samples of different runs are never pooled.
  - Scenario versions are never mixed: a changed scenario is a different case.
  - How many samples had no value is always shown (FR-12a).

## 4. Interfaces

- **REST API** (OpenAPI-specified): ingestion, retrieval, listing and running
  charts (FR-22), attachments, annotations, aliases.
- **MCP server:** read + annotate.
  - Read: list/search runs, get run details, list and run charts (FR-22), among
    them the built-in comparison and trend (FR-27), evaluate derived metrics, list
    scenarios/metrics/devices.
  - Write: add/edit tags and annotations only. No uploads, invalidation or
    deletion, and no saving of charts or dashboards.
- **Web UI:** run list, run detail (incl. attachments), charts and dashboards with
  their editor (FR-22 to FR-26), among them the built-in comparison, trend and
  series view (FR-27), faceted filtering, annotation editing, management of
  definitions, derived metrics and aliases, invalidation/deletion of runs.

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
    `write` (ingestion, including the tags a run is created with). Deletion and
    invalidation are **not** possible with any token; they are only available in
    the web UI. The same holds for saving, restoring and deleting charts and
    dashboards (FR-26). Listing and running them is possible with every scope.
  - Tokens are created and revoked in the web UI and stored only as hashes.
- **NFR-3 Volume:**
  - 1–5 large runs per day, or a few dozen small ones (e.g. one run per variant
    and device).
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

  The trend and the comparison are measured on the built-in charts (FR-27).

- **NFR-7 Long operations:** operations that may take longer than a few seconds
  (running a chart, exports) show progress in the UI. In the API they either stream
  results or run as jobs that can be polled.
- **NFR-8 Backup/restore** of the database and the attachment volume. Because hard delete
  is immediate (FR-10), backups are the only way to recover deleted runs. Backup
  retention must be documented.

## 6. Open Points

None. All open points have been decided. Remaining choices (storage format for
series blocks, DSL grammar, cache strategy) are design decisions, not
requirements.
