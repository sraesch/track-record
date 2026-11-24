# Concepts

The data model behind the API, in the terms the endpoints use.
[docs/requirements.md](../requirements.md) is the full specification; the
`FR-n`/`NFR-n` references point into it.

```text
run ─┬─ suite version, SUT version, shared environment, tags, attachments
     └─ case execution ─┬─ test case (scenario version + assets + parameters)
                        ├─ client device (+ device details)
                        └─ sample ─┬─ measurements  (metric → one value)
                                   └─ series        (metric → x/y points)
```

## Results

**Run.** One execution of a benchmark suite against one version of the
system under test (SUT). It records the suite version, the SUT version
(commit, branch, build configuration, tag), the shared environment, what
triggered it (`nightly`, `manual`, `ci`), the measured start and end time,
its status and its tags.

**Run status** (FR-2): a run is created `running` and ends as `completed`
or `failed` when the harness closes it. A running run without uploads for
the stale timeout becomes `aborted`. The final status never changes.

**Validity** (FR-9): runs are valid unless someone marks them invalid in
the web UI, with a reason. Invalid runs stay readable but are excluded from
lists and analysis unless a filter asks for them (`validity=invalid` or
`all`).

**Case execution.** One test case executed on one client device within a
run. It has its own measured start and end time and the device details at
that time. A run usually has many.

**Sample.** One repetition of a case execution, numbered by `repetition`.
A sample can be marked `is_warmup`; warmup samples are stored but left out
of aggregation by default (FR-12).

**Measurement.** One scalar value of one metric in one sample: a number, or
a boolean for boolean metrics.

**Series.** A sequence of points (x, y) of one metric in one sample, e.g.
frames per second over time. Each series names what x measures
(`x_dimension`, e.g. `time`) and its unit (`x_unit`). x never decreases but
may repeat (a jump). A sample has at most one series per metric.

**Attachment.** A file belonging to a run, and optionally to one of its
case executions or samples: logs, screenshots, traces. Stored as uploaded,
downloaded under its original file name.

**Immutability** (FR-6): uploaded results never change. A correction is a
new case execution that **supersedes** the old one; the old one is kept and
hidden by default.

## Definitions and dimensions

These are created implicitly by uploads and are shared between runs.

**Suite and scenario.** Named, free-form JSON definitions. Uploading a
definition that differs from every stored one creates a new **version**;
identical definitions map to the same version (FR-7). Results reference the
exact version used.

**Test case.** A scenario version, a set of assets and a flat set of
parameters (numbers, strings, booleans, with optional units for numeric
ones). The same combination is the same test case across runs.

**Case.** What trends and comparisons match on: a scenario version, its
assets and its parameters **after aliases**. Several test cases can form one
case when aliases make their parameters equal. A changed scenario definition
is a new scenario version and therefore always a different case.

**Asset.** An input of a test case (a model, a scene file), identified by
name and a version and/or content hash.

**Client device.** The device a case execution ran on, identified by a
**stable name** (`pixel-8-lab-1`). Its details (model, OS, driver, ...)
are stored per case execution as a device environment; when they change,
trends mark the change but are not split.

**Shared environment.** The details of what all case executions of a run
share (servers, backends). Identical details are stored once, identified by
a fingerprint (SHA-256 of the details).

**SUT version.** Commit, branch, build configuration and tag of the system
under test.

**Metric.** Name, unit, value type (`float`, `integer`, `boolean`),
direction (`higher_is_better`, `lower_is_better`) and description. A metric
has **one** definition and no versions (FR-7,
[ADR 0018](../adr/0018-metrics-without-versions.md)). Uploads create metrics
and never change them; the web UI can. Stored values are never converted:
after a unit change in the UI they are read in the new unit.

**Units** (FR-13a). Every metric, series x axis and numeric parameter has a
unit. Units from the predefined catalogue convert into each other within
their quantity (`ms` → `s`); other units are allowed but do not convert.
`""` is dimensionless.

**Aliases** (FR-14). Map a client device name or a parameter value to
another, e.g. a renamed device to its new name, so that results of both
form one trend. Aliases are versioned and set in the web UI. They apply
everywhere: filters, grouping, trends, comparisons, MCP. Responses show
both the uploaded value (`name`, `parameters`) and the value after aliases
(`resolved_name`, `resolved_parameters`).

## Analysis

**Derived metric** (FR-17). A named expression in a small, safe language
over the measurements and series of one sample, e.g.
`bytes_received / load_time`. It is computed when it is read, so it also
applies to old runs. A derived metric can have **no value** (a threshold
that was never reached), which is different from missing data: it is shown
and counted, but not used for differences (FR-12a). See
[Derived metrics](derived-metrics.md).

**Metric expression.** Every analysis view takes its metric as an
expression: an uploaded metric (`load_time`), a series (`series.fps`), a
derived metric (`derived.throughput`) or any expression
(`max(series.fps)`).

**Grouping.** Analysis groups samples by run × case × client device (after
aliases). Samples of different runs are never pooled.

**Trend comparison key** (FR-18). A trend follows one case and one client
device name (after aliases) across runs.
