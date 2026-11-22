# ADR 0018: Metrics without versions

**Status:** proposed · **Date:** 2026-11-22

## Context

Uploaded metrics were versioned (FR-7, [ADR 0004](0004-database-schema.md)):
every upload carried the definition of its metrics (unit, value type,
direction, description), a different definition became a new immutable
version, and measurements and series referred to the version they were
uploaded with.

That kept the unit of every stored value, but it made metrics different from
derived metrics, which have one definition that is edited in the web UI
([ADR 0017](0017-derived-metrics-without-versions.md)):

- A metric could only be changed by changing the harness. A typo in a
  description or a wrong direction needed a new upload.
- One metric could have several units at once. Analysis had to convert per
  sample, and the UI showed a version history and "load_time v2" columns.
- A metric could not be renamed or removed.

Metrics should be handled like derived metrics: one definition, which can be
changed, renamed and deleted in the web UI.

## Decision

- **A metric has one definition, which can be changed in the web UI.** There
  are no versions. FR-7 is changed accordingly. Suites, scenarios and aliases
  stay versioned.
- **Results refer to the metric, not to a definition.** A change applies to
  every result of the metric, historical ones included. Stored values are
  plain numbers and are never rewritten (FR-6): with another unit they are
  **read in that unit**. Changing `ms` to `s` turns a stored 1500 from
  1500 ms into 1500 s. The editor warns about this; it is the user's call.
- **Uploads create metrics, they never change them.** An upload still
  carries the definition of its metrics. If the metric does not exist, it is
  created with it. If it exists:
  - direction and description of the upload are ignored, so an upload cannot
    undo an edit;
  - another unit or value type is rejected (409,
    `metric_definition_conflict`, `TR009`). Accepting the upload would store
    its values under the wrong unit, and changing the metric from an upload
    would reinterpret all older values without anyone deciding it. After a
    unit was changed in the UI, the harness has to send the new unit.
- **Changing the unit checks the derived metrics that use the metric**,
  directly or through other derived metrics: the change is rejected (422,
  `breaks_derived_metrics`) if one of them is valid now and would not be
  afterwards. Like for derived metrics, the check and the save are not one
  transaction; evaluation checks units again per sample.
- **A metric can be renamed.** Its results stay with it. The expressions of
  the derived metrics that refer to it, as a scalar (`load_time`) or as a
  series (`series.load_time`), are rewritten in the same transaction; only
  those references change. Uploads identify a metric by name, so the harness
  has to send the new name: an upload under the old name creates a new
  metric. A name that another metric has is rejected (409); metrics are not
  merged.
- **A metric can be deleted when it has no results and no derived metric
  refers to it** (409, `metric_in_use`, `TR010` otherwise). Results are
  deleted only with their run (FR-8, FR-10), so this removes metrics that
  were created by mistake or whose runs were deleted.
- **Database** (migration `0013`): `metric` holds unit, value type,
  direction, description and `updated_at`. `measurement.metric_id` and
  `series.metric_id` replace `metric_version_id`; `metric_version` is
  dropped. `ensure_metric()` is what uploads call; `save_metric()`,
  `rename_metric()` and `delete_metric()` are for the web UI. Renaming and
  deleting take the lock that saves of derived metrics take, so the
  expressions that use the metric cannot change in between. Which derived
  metrics refer to a metric is read from their expressions by the backend;
  the database does not record it.
- **Upgrade:** the latest version of each metric becomes its definition, and
  the results of all its versions move to it. Results uploaded under an
  older version with another unit are read in the latest unit from then on.
  The result columns are rewritten by `ALTER TABLE`, with the immutability
  guard in place. The migration stops if a sample has values of two versions
  of one metric, which cannot be merged (one value per sample and metric);
  the runs with such samples have to be deleted first.
- **API:** `GET /metrics` returns one object per metric with `unit`,
  `value_type`, `direction`, `description`, `created_at`, `updated_at`,
  `has_results` and `used_by` (names of the derived metrics that refer to
  it). Measurements and series carry `metric_id` instead of
  `metric_version_id` and `metric_version`.
  `PUT /ui/api/v1/metrics/{name}` changes a metric (404 if there is none:
  only uploads create metrics), `POST /ui/api/v1/metrics/{name}/rename`
  (`{"name": …}`) returns the metric and the names of the derived metrics
  whose expressions were `rewritten`, `DELETE /ui/api/v1/metrics/{name}`
  returns 204. All three exist only on the UI port. MCP stays read-only for
  metrics.
- **UI:** the metrics page lists one definition per metric with "Edit",
  "Rename" and "Delete". The edit dialog says that a change applies to all
  results, warns when the unit or value type of a metric with results
  changes, and names the derived metrics a new unit reaches. The rename
  dialog names the expressions it changes and warns about the harness and
  about views using the old name. The delete dialog cannot be confirmed
  while the metric has results or derived metrics use it. The case
  execution page shows one column and one chart per metric.

## Consequences

- **The unit of a stored value is no longer recorded with it.** A unit
  change silently changes what historical values mean; there is no history
  and no undo. If the harness starts measuring in another unit, the old and
  the new values cannot both be right under one definition: either the
  harness uses a new metric name, or the old values are accepted as wrong.
  This was chosen deliberately for simplicity; versions were the mechanism
  that prevented it.
- A changed harness can be blocked: after it switches units, its uploads are
  rejected until the metric is changed in the UI (or the other way round).
  The error says so.
- Uploads from before a unit change that arrive late, or imports of old
  data, are rejected if they still send the old unit.
- A view URL with a metric name no longer shows the same numbers after the
  metric was changed, and stops working after it was renamed (FR-21
  reproduces the view, not the definitions it had then).
- The last save wins if two people edit the same metric. The app has no user
  identity, so there is nothing to attribute changes to.
- Renaming rewrites saved derived metrics only. Ad-hoc expressions in view
  URLs keep the old name.
- Less code and a simpler UI: no version history, no per-version columns
  and charts, one unit per metric in every analysis.
