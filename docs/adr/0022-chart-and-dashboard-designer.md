# ADR 0022: Chart and dashboard designer

**Status:** proposed · **Date:** 2026-11-27

## Context

Analysis is a fixed set of views today: aggregates, trend, comparison,
series overlay and bands, and export, each with its own view spec and
endpoint ([ADR 0009](0009-derived-metric-dsl-and-analysis.md)). Their pages
in the web UI (Compare, Trends, Series) are not built yet. The views cannot
answer questions such as:

- a metric over a case parameter, one line per device (FR-15);
- one metric against another, per sample;
- one score per run across all cases, e.g. the geometric mean of the ratios
  to a reference release;
- many trends on one page.

The concept for a designer answers them in two steps: a graph of operations
prepares the data, and a visualization shows the result. It was reviewed on
2026-11-27 ([issue 23](https://github.com/sraesch/track-record/issues/23)).
The decisions of that review are requirements now: FR-22 to FR-27, and
changes to FR-7, FR-11, FR-12a, FR-16 to FR-21 and NFR-2. This ADR says how
they are built. The decisions that shape the design:

- A chart is **one node graph** with one visualization. A dashboard arranges
  charts.
- The smallest unit of data is **one series of one sample**. A scalar is a
  series with one point.
- The rules of the fixed views (never pool samples of different runs, never
  mix scenario versions in a trend, always show how many samples had no
  value) hold in the **built-in charts only**.
- Results of different runs can be related with **free math** between
  matched branches of a graph.
- Charts are **run through REST and MCP** and saved only in the web UI,
  with a **history**.
- The built-in charts and one endpoint **replace** the Compare, Trends and
  Series pages and the analysis endpoints.

## Decision

### Frames, points and groups

- A **frame** is an ordered list of **points**. All points of a frame have
  the same **columns**. A column has a name and a unit, and its values are
  numbers. A value can be **no value** (FR-12a); the point stays.
- A series of one sample is a frame with the columns `x` and `y`. `x` keeps
  the x-dimension and unit of the series. A scalar is a frame with one point
  and the column `y`.
- Frames carry **attributes** (see below). A point can carry attributes of
  its own; where it has none for an attribute, the frame's value counts.
- A **group** is an ordered list of frames, or an ordered list of groups,
  never both. A group below the root has a **key**: the attributes it was
  grouped by, with their values. All frames are at the same depth.
- The output of every node is such a tree. The output of a source is the
  root with a flat list of frames.
- When frames are merged into one (reduce, concatenate, combine), the new
  frame keeps the attributes that all merged frames share. After a reduce
  or a concatenate, every point also keeps the attributes of the frame it
  came from, so any attribute can become an axis.

### Attributes

Attributes are the dimensions of the domain model, under fixed names:

| Family | Attributes |
|---|---|
| `run` | `run.id`, `run.started_at`, `run.ended_at`, `run.status`, `run.trigger`, `run.valid`, `run.tags`, `suite.name`, `suite.version`, `sut.id`, `sut.commit`, `sut.branch`, `sut.build_config`, `sut.tag`, `environment.id` |
| `case` | `scenario.name`, `scenario.version`, `asset.<name>`, `param.<name>` |
| `device` | `device.name`, `device.details` |
| `sample` | `case_execution.id`, `sample.id`, `sample.repetition`, `sample.warmup` |
| `source` | `source` (the id of the source node), `metric` (its expression) |
| derived | `attr.<name>`, added by a derive node |

- `device.name` and `param.<name>` are the values **after aliases** (FR-14).
  Parameters keep their JSON type and their unit.
- `device.details` is the id of the device details of a case execution.
  They change without changing `device.name`.
- Where a node takes a list of attributes as a **key**, a family name
  stands for all its attributes, and `except` takes attributes out again:
  `["case", "device.name"]` except `["param.codec"]`.
- The family `case` is the case of the domain model: scenario version,
  assets and parameters after aliases. Grouping by `scenario.name` alone can
  mix scenario versions; grouping by `case` cannot.

### Sources

A source is a run filter (FR-11, with the relative selection of runs), one
metric expression in the DSL (FR-17a) and the options `unit`, `x_unit` and
`include_warmup`, as in today's view specs.

- It returns **one frame per sample** for which the expression can be
  evaluated: the points of a series, or one point for a scalar. A derived
  scalar without a value is one point with no value.
- Invalid runs and warmup samples are left out unless the filter
  (`validity`) or `include_warmup` says otherwise. Superseded case
  executions are never used. Aliases are applied before any node sees a
  value.
- Samples that lack an input of the expression, or whose inputs do not fit
  it, get no frame. They are counted per source in `excluded`, as today.
- Frames are ordered by measured start of the run, run, case execution and
  repetition.
- A graph can have several sources, e.g. two metrics or a reference run.

Sources use what exists: the run filter, the selection of samples and the
evaluation of the expression per sample (`service::derived`), with series
read through the series store.

### Nodes

| Node | Inputs | What it does |
|---|---|---|
| `source` | none | A flat list of frames, one per sample |
| `filter` | 1 | Keeps the frames, or the points, that meet all conditions. Frames and groups that become empty are removed |
| `derive` | 1 | Adds an attribute that is computed from other attributes |
| `group` | 1 | Splits the frames of every innermost group by a key: one more level |
| `flatten` | 1 | Removes the innermost level (or several, or all). The frames move up, in order |
| `sort` | 1 | Orders frames, the groups of the innermost level, or the points of each frame, by attributes or columns |
| `select_latest` | 1 | Keeps, in every innermost group, the frames of the latest N runs; or, in every frame, the points of the latest N runs |
| `resample` | 1 | Puts frames with an `x` column onto a common x grid |
| `match` | 2 or more | Pairs each frame of the first input with the frames of the other inputs that have the same key |
| `reduce` | 1 | Turns every frame into one point. The frames of each innermost group become one frame |
| `concatenate` | 1 | The frames of each innermost group become one frame with all their points |
| `combine` | 1 | The frames of each innermost group become one frame, point by point |
| a chart type | 1 | The end of the graph: the visualization |

`reduce`, `concatenate` and `combine` replace every innermost group by one
frame, so that level disappears. On a flat list the result is one frame.

- **filter:** a condition is an attribute (for points also a column), an
  operator (`=`, `!=`, `in`, `not in`, `<`, `<=`, `>`, `>=`, `exists`) and a
  value. Values keep their JSON type: `10` is not `"10"`. No value never
  meets a comparison.
- **derive:** `name`, `expression` and an optional `unit`. The expression
  uses the DSL's grammar, operators and scalar functions, with its unit
  rules. Its names are numeric attributes (`param.width * param.height`),
  not metrics. The result is `attr.<name>`. A frame that lacks an attribute
  of the expression does not get the new one.
- **group:** `by` is a key. The subgroups are ordered by their key values;
  the frames keep their order. Frames without a value for the key form a
  subgroup of their own.
- **select_latest:** the latest runs by measured start, then by run id, as
  the run list orders them.
- **resample:** a grid (`points`, `from`, `to`) and `interp` (`linear` or
  `step`), with the meaning they have in the series views today (FR-19).
  Without `from` and `to`, the grid spans the frames of each innermost
  group.
- **match:** the inputs have names (`a`, `b`, …). The key is all attributes
  of a frame of `a`, except the ones named in `except`, or only the ones in
  `only`. For every frame of `a`, the node looks up the frame with the same
  key values in each other input. Another input may hold at most one frame
  per key; more is an error that names the key. The output is the tree of
  `a`, in which a group replaces each frame: it holds the frame and its
  partners under the names of their inputs. A frame without a partner is
  dropped, or kept with `unmatched: keep`. The attributes of the family
  `source` are never part of the key, so that frames of two sources can
  match. This is how results of different runs are related: the key leaves
  out the attributes in which they differ (FR-13, FR-16).
- **reduce:** `column` (by default the only column that is not `x` or a
  count) and a list of functions. Each function gives a column of its name
  (`mean`, `p95`, `ci_low`); `n` and `n_no_value` are always added. Points
  without a value are left out of the functions and counted (FR-12a). The
  new points are in the order of the frames they come from.
- **concatenate:** the frames need the same columns.
- **combine** has two forms:
  - **A function** across all frames of the group, point by point
    (`mean`, `median`, `min`, `max`, `sum`, `percentile`, `count`), e.g. a
    median with a percentile band (FR-19).
  - **Expressions** over the frames of a match, named by their inputs:
    `a - b`, `(a - b) / abs(b)`, `a.mean / b.mean` where frames have several
    columns. They use the DSL's grammar, its operators, its point-by-point
    functions and its unit rules, and can read numeric attributes. The new
    frame has the columns of the inputs (`a`, `b`, or `a.mean`, `b.mean`)
    and one column per expression. It has the attributes of `a`; those of
    the other inputs are kept as `b.<attribute>`. A missing partner or no
    value gives no value: the difference is not computable (FR-12a). The
    list of expressions can be empty. The node then only puts the inputs
    side by side, e.g. for a scatter plot of one metric against another.

  Frames are combined point by point when they have the same `x` values or
  no `x` and the same number of points. A frame with one point and no `x`
  applies to every point of the others, as a scalar does in the DSL.
  Different `x` values are an error that names `resample`: resampling is
  never implicit.

**Functions of reduce:** `mean`, `median`, `min`, `max`, `sum`, `stddev`,
`percentile` (with `p`), `ci_low` and `ci_high` (confidence interval of the
mean; `confidence`, 0.95 by default), `geomean`, `first`, `last`, `count`.
They are computed as today (`track-record-core::stats`): sample standard
deviation, linearly interpolated percentiles, Student's t. A function
without a result, e.g. the standard deviation of one point or the geometric
mean of a negative number, gives no value. All keep the unit of the column
except the counts, which have none.

The lists of functions are fixed. A graph has no loops, no recursion and no
code other than these nodes and the DSL expressions inside them, so it is
as safe to evaluate as a derived metric (FR-17a).

### Visualizations

The last node of a graph is its visualization. Its settings assign columns
and attributes to **roles**:

| Type | Draws | Roles |
|---|---|---|
| `line` | One line per frame | `x`, `y`, optional band (`low`, `high`) |
| `scatter` | One dot per point | `x`, `y` |
| `bar` | One bar per point | `category`, `value`, optional error (`low`, `high`) |
| `box` | One box per frame, from its points | `category`, `value` |
| `heatmap` | One cell per point | `x`, `y`, `value` |
| `value` | One number | `value`, optional `compare` |
| `table` | One row per point | A list of columns and attributes; optional `across` for a pivot |

- A role takes a column or an attribute: `x: "run.started_at"`,
  `x: "param.model_size"`, `category: "device.name"`. The axis is a time,
  number or category axis by the type of the value. A category axis keeps
  the order of first appearance.
- **Group levels** are assigned to `panel` (one small chart per group) and
  `color`, from the outermost level on. A graph that reaches the
  visualization with more levels than it has roles is invalid: flatten or
  merge first.
- An axis can convert its unit within the catalogue and use a log scale
  (FR-13). Labels come from the metric, the unit and the attributes.
- `line` can **mark changes** of chosen attributes from one point to the
  next (`sut.id`, `environment.id`, `device.details`) and show the
  **annotations** of the runs, test cases and time range it covers (FR-18).
  The service adds the annotations to the result; nodes do no I/O.
- `table` can mark where an attribute differs from the same attribute of a
  matched partner (`sut.id` and `b.sut.id`, FR-16).
- The frontend builds the ECharts option of each type in a pure function
  ([ADR 0015](0015-charting-library.md)).

### Variables

- A chart declares **variables**: a name, a type (`text`, `number`,
  `boolean`, `time`, `metric`, `run_filter`, `attribute`, or a list of
  one of these) and an optional default.
- Any value in the settings of a node can be `{"var": "<name>"}`, also
  inside a run filter. The type must fit the setting. A variable replaces a
  value; it cannot add or remove nodes.
- A run gets the values of the variables. A missing value falls back to the
  default; without a default the run is rejected.
- A **dashboard** has variables of its own and passes them to its charts:
  by default to the variables of the same name, or as the placement of the
  chart says.
- The URL of a chart or dashboard carries the values (`?v.branch=main`),
  so it reproduces the view (FR-21).

### Spec

A chart is stored and sent as JSON:

```json
{
  "format": 1,
  "variables": [{"name": "branch", "type": "text", "default": "main"}],
  "nodes": [
    {"id": "runs", "type": "source", "metric": "load_time",
     "filter": {"suite": "web-viewer", "sut_branch": {"var": "branch"}}},
    {"id": "lines", "type": "group", "input": "runs",
     "by": ["case", "device.name"]},
    {"id": "per_run", "type": "group", "input": "lines", "by": ["run.id"]},
    {"id": "samples", "type": "concatenate", "input": "per_run"},
    {"id": "stats", "type": "reduce", "input": "samples",
     "functions": ["mean", "ci_low", "ci_high"]},
    {"id": "chart", "type": "line", "input": "stats",
     "x": "run.started_at", "y": "mean", "low": "ci_low", "high": "ci_high"}
  ]
}
```

This is a trend: `samples` holds one frame per run, case and device with a
point per sample, and `stats` turns each into one point, which leaves one
frame per case and device with a point per run.

- `format` is the version of the spec format, so that it can change later.
- A node has an `id`, a `type`, its `input` (or named `inputs` for a match)
  and its settings. The editor keeps the position of a node in the spec;
  evaluation ignores it.
- **Well-formed** means: known node types and settings, unique ids, inputs
  that exist, no cycle, exactly one visualization. Only a well-formed spec
  can be saved.
- **Valid** means in addition: the expressions pass the DSL's checks,
  attributes and variables exist, columns and units fit from node to node,
  and the group levels fit the visualization. Validation reports problems
  per node and setting; for expressions with the positions the DSL gives
  today. A chart with such problems can be saved, because a later change to
  a metric can cause them anyway. It cannot be run.
- **Limits:** 100 nodes and 8 sources per graph; the DSL's limits for every
  expression; 2,000,000 samples per source, as for an analysis today; and a
  budget of points that one run may hold in memory (10 million by default,
  configurable). A run over the budget fails and says how to stay below it:
  narrow the filter, or reduce the series in the metric expression
  (`max(series.fps)`).

### Format 1

The settings that the sections above leave open, as
`track-record-core::chart` reads them:

- **Functions** of reduce and combine are a name (`"mean"`) or an object
  with a parameter (`{"function": "percentile", "p": 95}`). The column is
  the name, `p<p>` for a percentile. `ci_low` and `ci_high` use the
  `confidence` of the reduce node (0.95 by default).
- **combine** has either `functions` (with `column`) or `expressions`, each
  `{"name", "expression", "unit"}`.
- **match** names its inputs in `inputs`:
  `{"a": "runs", "b": "reference"}`. A name is an identifier, not `x`, and
  not the start of attribute names (`sut`, `param`, ...), so that `b.sut.id`
  is clear after a combine. Such attributes keep the family of the
  attribute they come from: a later key with `except: ["run"]` also leaves
  out `b.run.id`, and `b.source` is never part of a match key.
- **Visualizations** assign group levels with `levels` (outermost first;
  `["panel", "color"]` by default), convert units and use log scales with
  `axes` (`{"y": {"unit": "s", "log": true}}`). `line` marks changes with
  `marks` and shows annotations with `annotations`; `table` marks
  differences to matched partners with `differs`.
- **Variables** are `{"name", "type", "list", "default"}`. A variable stands
  for a whole setting or for one field of a run filter, not for an element
  of a list or for the structure of a node (inputs, conditions, functions,
  expressions). These settings can take one: the metric, filter, filter
  fields, units and `include_warmup` of a source; condition values; keys
  (`by`, `except`, `only`); `runs`, `points`, `from`, `to`, `confidence`;
  `column`; and the roles, role lists and axis units of visualizations.
- **Validation** infers for every node its group levels, its columns with
  their units, and the attributes its frames and points can have. A setting
  that is a variable is checked with its default; without one, what depends
  on it is known only when the chart runs. Until the DSL reads the input
  names of combine expressions, the unit of an expression column is the
  declared one.
- **Order of values:** group orders its subgroups, and sort its frames,
  groups or points, by value: numbers, then times, texts, booleans and
  lists. Numbers compare by value, as the analysis compares parameters:
  `10` equals `10.0`, exactly also for integers beyond the precision of
  `f64`. Keys compare value and unit, so a parameter of `10 Mbit/s` and one
  of `10 kbit/s` are two cases. Frames without a value for a key form the
  last subgroup; missing values sort last, also with `desc`. A group key
  holds the attributes `by` names, also without a value, and of the
  attributes a family stands for those the frames have, in the order of
  the table above, then `asset.*` and `param.*` by name.
- **filter:** a missing attribute and no value meet no condition, `!=` and
  `not in` included. On `run.tags`, `=` means the list contains the value,
  `in` that it contains one of the values, and `!=` and `not in` the
  opposite. Comparisons take numbers and times (RFC 3339).
- **select_latest:** frames, or points, without `run.id` are dropped.
- **derive:** the expression reads attributes by name (`param.width`) and
  calls only the scalar functions `abs`, `log10`, `sqrt`, `min` and `max`
  of two or more values, and `clamp`, with their argument counts checked
  when it is read; metric expressions keep rejecting attribute names. The
  unit of a square root (`sqrt(px^2)`) reads back as that unit in a later
  derive. The units of
  parameters and derived attributes come from the data, so a run checks the
  expression for each frame: a frame whose units do not fit, or for which
  the expression has no value, does not get the attribute, as one without
  an input. Validation checks units only where all attributes are fixed
  ones, which have none. After a concatenate, points with attributes of
  their own get a value of their own. A unit the expression builds may
  have at most 256 characters (chained derives such as `attr.a * attr.a`
  double it); a longer one fails the node and asks for a declared `unit`.
  A unit it only passes on, from an attribute or declared, may be longer.
- **Sources:** the value of `asset.<name>` is the asset's version when the
  case has one asset of that name and it has only a version (`"v1"`). One
  asset with a content hash is a list of `version=<version>` and
  `hash=<content hash>` (`["version=v1", "hash=abc"]`), and several assets
  of one name are a list of such lists, so that distinct assets never give
  the same value. `sut.tag` is a text, as
  a SUT version has one tag. The `x` column keeps the x-dimension of the
  series; frames with different ones are not concatenated. A derived series
  without a value is a frame without points. The budget of points is
  `TR_CHART_MAX_POINTS` (10 million); input series are loaded in batches of
  at most a million points, and a sample whose input series alone have more
  points than the budget fails before they are loaded. Status, validity and
  end of a run are read from one row, so they fit together.
- **Empty results** lose their group levels; nodes that need a level
  (sort of groups) pass an empty result on.

### Evaluation

- Charts run **on the server**, in a new `service::charts` that the REST
  handlers of both ports and the MCP tools share. The node logic is pure
  code without I/O in `track-record-core`, next to units, statistics,
  series and the DSL. Only sources read data.
- A run validates the spec, loads the sources in chunks and evaluates the
  nodes in the order of their inputs. A node with several consumers is
  evaluated once.
- The response is an **NDJSON stream** like today's comparison (NFR-7):
  `start`, `progress` (samples done and in total), one `result`, then
  `done` or `error`. Request errors are problem details before the stream
  starts. The result is the tree that reaches the visualization: groups
  with their keys, frames with their columns and units, points with their
  values and attributes, and `excluded` per source.
- The result arrives at the end, not row by row: most nodes need all their
  input. Progress counts the samples of the sources, which is where the
  time goes.
- Lines are downsampled for display on the server (`max_points`), as today.
- **Preview:** a run can stop at a node (`until`) and be limited to part of
  the data (`preview`): each source then reads only the latest run that
  matches its filter. The result is marked `partial`. The editor uses this
  for the selected node.
- **Export** (FR-20): with `format` `csv`, `json` or `parquet`, a run
  streams the data that reaches the visualization as a file, one row per
  point with its attributes and columns.
- There is no cache (NFR-5 still allows one), and nothing is kept between
  requests (NFR-1).

### Data rules and built-in charts

- The engine does not enforce the three rules of the fixed views, and
  nothing marks a chart that breaks them.
- The **built-in charts** `trends`, `compare` and `series` are specs that
  ship with the server. They are listed with the saved charts, cannot be
  changed or deleted, and can be duplicated as own charts. Their variables
  are the fields of today's view specs.
- They keep the rules by how they are built:
  - They group by `run.id` before the first node that merges frames, so
    samples of different runs are never pooled. Runs meet only as points
    of a frame or through a match.
  - They group and match by `case`, never by `scenario.name`.
  - Their tables and tooltips show `n` and `n_no_value`, and a difference
    against no value is shown as not computable.
- Tests hold them to that. Until the analysis endpoints are removed, each
  built-in chart and its endpoint run on the same data and must return the
  same numbers.

### Storage and history

- `chart` and `chart_revision`, `dashboard` and `dashboard_revision`. A
  revision holds title, description and the spec (`jsonb`); for a dashboard
  the placements of its charts and its variables. The current definition is
  the latest revision. Ids are `bigint` identities, as for the other
  definitions ([ADR 0004](0004-database-schema.md)); the built-in charts
  are addressed by their names.
- Revisions are numbered per chart and dashboard from 1; a save names the
  number it is based on.
- A **placement** puts a chart on a dashboard:
  `{"chart": {"id": 12} | {"builtin": "trends"}, "layout": {...}, "variables": {...}}`.
  `layout` is the position and size on the grid of the dashboard page.
  `variables` maps a variable of the chart to a value or to a variable of
  the dashboard (`{"var": "<name>"}`); a chart variable it leaves out gets
  the dashboard variable of the same name. A chart can be placed more than
  once, with other variables. The database checks only the reference to
  the chart and that a saved one exists; the backend checks the rest.
- `dashboard_chart` records the saved charts that the **current** revision
  of each dashboard places. Its foreign key keeps them from being deleted.
  An old revision may place a chart that was deleted since; restoring it is
  rejected until the placement is removed.
- **Every save adds a revision**, also one with unchanged content. Revisions never change. Restoring saves
  the content of an old revision as a new one, so the history only grows.
- A save names the revision it is based on and is rejected if that is no
  longer the latest (409, `chart_changed`). The app has no user identity,
  and without this check the last save would silently win.
- Deleting a chart removes its revisions. A chart that the current revision
  of a dashboard uses cannot be deleted (409, `chart_in_use`). Deleting
  removes definitions, not results (FR-8).
- Writes go through database functions (`save_chart()`, `delete_chart()`
  and the same for dashboards), like the other definitions.
- Which metrics a chart uses is read from the expressions of its current
  revision by the backend, as for derived metrics (`metric_users`).
  Renaming a metric or derived metric saves a new revision of each chart
  that uses it, with the expressions rewritten (`dsl::rename_metric`). A
  metric or derived metric that a chart uses cannot be deleted. Other
  changes to a metric (unit, value type) can leave a chart invalid; the
  chart then says so.

**Why a history here**, when metrics and derived metrics have none
([ADR 0017](0017-derived-metrics-without-versions.md),
[ADR 0018](0018-metrics-without-versions.md)): versions were removed there
because results or other definitions pinned them, which made a change not
reach what was built on it. Nothing refers to a chart revision; the history
exists only to restore. And a graph is far more work to rebuild than an
expression, while everyone with access can change it.

### Interfaces

On the machine port (`/api/v1`, any token scope) and on the UI port:

| Endpoint | Purpose |
|---|---|
| `GET /charts`, `GET /charts/{id}` | List the charts, built-in ones included; one chart with its spec and revision |
| `POST /charts/{id}/run` | Run a saved chart with values for its variables |
| `POST /charts/run` | Run a spec that is not saved |
| `POST /charts/validate` | Check a spec: problems and the columns of each node |
| `GET /dashboards`, `GET /dashboards/{id}` | List the dashboards; one dashboard |

Only on the UI port (NFR-2): `POST /charts`, `PUT /charts/{id}`,
`DELETE /charts/{id}`, `GET /charts/{id}/revisions`,
`POST /charts/{id}/restore`, and the same for `/dashboards`.

- **MCP** gets the read tools `list_charts`, `get_chart`, `run_chart` (a
  saved chart or a spec), `validate_chart`, `list_dashboards` and
  `get_dashboard`, for every scope. `run_chart` collects the stream into
  one result, as `compare` does today. There are no tools to save.
- **Replaced:** `/analysis/aggregates`, `/analysis/trend`,
  `/analysis/compare`, `/analysis/series/overlay`,
  `/analysis/series/aggregate` and `/analysis/export` on both ports, and
  the MCP tools `aggregates`, `trend` and `compare`. The section "Analysis"
  of ADR 0009 ends with them; its DSL sections stay. The endpoints for
  validating and evaluating derived metrics stay.
- **Order of the switch:** the old endpoints stay until the built-in charts
  return the same results in tests. They are removed in one release, with
  the REST documentation and a "Removed" entry in the changelog. It is a
  breaking change of the machine API of 0.1.0.

### Web UI

- Routes: `/charts`, `/charts/:id`, `/charts/:id/edit`, `/dashboards` and
  `/dashboards/:id`. `/compare`, `/trends` and `/series` stay and show the
  built-in charts.
- The editor is a node graph with the preview of the selected node. Its
  library gets its own ADR with the editor.
- A graph that is not saved has no URL and cannot be shared (FR-21). A URL
  always shows the current definition of a chart.

## Consequences

- An own chart can pool runs, mix scenario versions or hide how many
  samples had no value, and nothing in the app or in the result of
  `run_chart` says so. The numbers of the built-in charts are the ones the
  rules protect.
- The result of a run arrives in one piece, with progress before it.
  Today's comparison sends its rows as they are computed; that is lost.
- Memory per run is bounded by the point budget. Charts over raw series of
  whole runs do not fit; they have to reduce in the metric expression.
- A saved chart can stop working when a metric changes. Renames are
  followed, deletes are blocked, other changes are reported by the chart.
- A URL shows the current definition, not the one the chart had when the
  link was made. The history can show what changed.
- Variables cannot change the shape of a graph. A built-in chart with an
  optional part (the reference of a trend) keeps the part and lets it yield
  no value.
- The DSL gets a second kind of name, attributes, which only derive and
  combine expressions accept. Metric expressions stay within one sample
  and cannot read attributes (FR-17). The tokenizer of the expression
  editor has to follow ([ADR 0016](0016-expression-editor.md)).
- The machine API breaks once, between 0.1.0 and the release that removes
  the analysis endpoints.
- This is far more to build and to test than the three fixed pages: an
  engine, a spec format that has to stay readable for old revisions, and
  an editor.

## Left for later

- The library of the node editor (its own ADR).
- Evaluating a filter or a reduce while a source loads, which would lift
  the point budget for the common cases without changing specs; a cache of
  results (NFR-5).
- Attributes from data (e.g. grouping by a reduced value) and mapping the
  values of text attributes in a derive node.
- Parameters in metric expressions.
- Several inputs to one visualization (layers).
