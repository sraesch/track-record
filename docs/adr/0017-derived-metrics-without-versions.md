# ADR 0017: Derived metrics without versions

**Status:** proposed · **Date:** 2026-11-22

## Context

Derived metrics were versioned like the other definitions (FR-7,
[ADR 0004](0004-database-schema.md)): every save added an immutable version,
and a version pinned the exact versions of the derived metrics it used
([ADR 0009](0009-derived-metric-dsl-and-analysis.md)).

Versions of suites, scenarios and metrics exist because results refer to
them. Nothing refers to a derived metric version: derived metrics are
evaluated at query time (FR-17), and an analysis always used the latest
version of the metric it named. What the versions added:

- A changed expression did not reach the metrics built on it until each of
  them was saved again. The UI had to mark such outdated pins.
- A version history, a version selector and "new version" instead of "edit"
  in the UI, the API and MCP.

Derived metrics should be editable instead: one definition that can be
changed.

## Decision

- **A derived metric has one definition, which can be changed.** There are no
  versions. Requirements FR-7 and FR-17 are changed accordingly.
- **A change applies everywhere from then on:** to every analysis, historical
  runs included, and to the derived metrics built on the changed one.
  `derived.<name>` always means the current definition.
- **The name is how expressions refer to a derived metric.** It can be
  changed: renaming also rewrites `derived.<old>` to `derived.<new>` in the
  expressions of the derived metrics that use it, in the same transaction,
  so they keep working. Only those references are replaced; the rest of
  each expression stays as it was written.
- **A derived metric can be deleted when no other derived metric uses it.**
  Otherwise the delete is rejected (409, `derived_metric_in_use`) and names
  the metrics to change or delete first. Deleting removes a definition, not
  data: uploaded results are kept (FR-8).
- **Saving checks the metrics built on the saved one.** In addition to the
  checks of FR-17a (syntax, references, no cycle, units), a change is
  rejected if a derived metric that uses the saved one, directly or through
  others, is valid now and would not be afterwards. The same check runs while
  typing (`POST /derived-metrics/validate` with `name`). A metric that is
  already invalid for another reason does not block the change.
- **Database** (migrations `0011` and `0012`): `derived_metric` holds
  expression, unit, description and `updated_at` and can be updated.
  `derived_metric_dependency` maps a derived metric to the derived metrics
  its expression uses and is replaced with the expression. It goes with the
  metric when that is deleted; a used metric cannot be deleted (foreign
  key). `rename_derived_metric(name, new_name)` and
  `delete_derived_metric(name)` take the same lock as saves; the latter
  raises `TR008` while other derived metrics use the metric.
  `save_derived_metric(name, expression, unit, description, uses)` creates or
  changes the metric, takes the used metrics by name, rejects cycles
  (`TR007`) and returns the id and whether the metric is new. It locks the
  table against other saves, so two saves cannot close a cycle together;
  reads are not blocked.
- **Upgrade:** the latest version of each derived metric becomes its
  definition. Older versions are dropped. A metric that pinned an older
  version of another one uses that metric's definition from then on. The
  migration stops if the latest versions use each other in a circle, which
  pinned versions allowed (A v2 uses B v1, B v2 uses A v1); one of them then
  has to be saved without the other before migrating.
- **API:** `PUT /ui/api/v1/derived-metrics/{name}` creates (201) or changes
  (200) the metric. `GET /derived-metrics` returns one object per metric with
  `expression`, `unit`, `description`, `created_at`, `updated_at` and `uses`
  (names). `evaluate` no longer takes a `version`.
  `POST /ui/api/v1/derived-metrics/{name}/rename` (`{"name": …}`) returns the
  metric and the names of the metrics whose expressions were `rewritten`;
  `DELETE /ui/api/v1/derived-metrics/{name}` returns 204. Like saving, both
  exist only on the UI port. MCP stays read-only for derived metrics.
- **UI:** the page shows the definition with "Edit", "Rename" and "Delete".
  The editor says that a change applies to all analyses and names the
  derived metrics it also changes. "New" does not replace an existing
  metric. The rename dialog names the expressions it changes and warns that
  views using the old name stop working; the delete dialog asks first and
  cannot be confirmed while other derived metrics use the metric.

## Consequences

- A view URL with `derived.<name>` no longer shows the same numbers after the
  metric was changed (FR-21 reproduces the view, not the definition it had
  then). To keep old numbers comparable, save the changed expression under a
  new name instead, or put the expression itself into the view.
- There is no history and no undo: the previous expression is gone after a
  change, and the last save wins if two people edit the same metric. The app
  has no user identity, so there is nothing to attribute changes to either.
- Changing a metric can change the meaning of metrics built on it without
  making them invalid, e.g. a number without a unit next to a metric whose
  unit changed (`derived.a + 1`). The editor names the metrics a change
  reaches, but only checks that they stay valid.
- The check and the save are not one transaction. Two changes made at the
  same moment can leave a metric that no longer fits one it uses. Evaluation
  checks units again per sample and reports it there; cycles are ruled out by
  the database.
- Renaming or deleting a derived metric breaks analysis view URLs and
  ad-hoc expressions that use its name; only saved derived metrics are
  rewritten. The dialogs say so.
- Less code and a simpler UI: no pinning, no version history, no outdated
  pins.
