import type { DerivedMetric, EvaluateResponse } from '../api/client'

// The derived metrics view (FR-17). Its state lives in the URL (FR-21):
//
//   /derived-metrics?q=fps                        list filtered by the search
//   /derived-metrics?name=throughput              a derived metric selected
//   /derived-metrics?edit=new                     editor for a new metric
//   /derived-metrics?name=throughput&edit=change  editor for the selected one
//   ...&run=<id>&case=<id>                        the editor's preview case
//
// A derived metric has one definition, which can be changed (ADR 0017):
// there are no versions to select.

export type DerivedEdit = 'new' | 'change'

export type DerivedView = {
  /** Text filter of the list, trimmed; undefined when empty. */
  search?: string
  /** The selected derived metric. */
  name?: string
  /** The editor in the right panel. */
  edit?: DerivedEdit
  /** Run and case execution the editor previews the expression on. */
  run?: string
  caseExecution?: string
}

export function parseDerivedView(params: URLSearchParams): DerivedView {
  const name = params.get('name') || undefined
  const edit = params.get('edit')
  return {
    search: params.get('q')?.trim() || undefined,
    name,
    // A change needs the metric to change.
    edit:
      edit === 'new' ? 'new' : edit === 'change' && name ? 'change' : undefined,
    run: params.get('run') || undefined,
    caseExecution: params.get('case') || undefined,
  }
}

/** Changes the view; `replace` replaces the history entry, `state` goes with it. */
export type SetDerivedView = (
  view: DerivedView,
  options?: { replace?: boolean; state?: unknown },
) => void

/** The view as a URL query; what is not set is left out. */
export function derivedViewToParams(view: DerivedView): URLSearchParams {
  const params = new URLSearchParams()
  if (view.search?.trim()) params.set('q', view.search.trim())
  if (view.name) params.set('name', view.name)
  if (view.edit) params.set('edit', view.edit)
  if (view.run) params.set('run', view.run)
  if (view.caseExecution) params.set('case', view.caseExecution)
  return params
}

/** The derived metrics sorted by name. */
export function sortDerived(metrics: readonly DerivedMetric[]): DerivedMetric[] {
  return [...metrics].sort((a, b) => a.name.localeCompare(b.name))
}

/** Derived metrics whose name, expression, unit or description contain `search`. */
export function filterDerived(
  metrics: readonly DerivedMetric[],
  search: string | undefined,
): readonly DerivedMetric[] {
  const needle = search?.trim().toLowerCase()
  if (!needle) return metrics
  return metrics.filter(({ name, expression, unit, description }) =>
    [name, expression, unit, description].some((text) =>
      text.toLowerCase().includes(needle),
    ),
  )
}

/** Names of the derived metrics whose expression uses `name`, sorted. */
export function derivedUsers(
  metrics: readonly DerivedMetric[],
  name: string,
): string[] {
  return metrics
    .filter((metric) => metric.uses.includes(name))
    .map((metric) => metric.name)
    .sort((a, b) => a.localeCompare(b))
}

/**
 * Names of all derived metrics built on `name`, directly or through
 * others, sorted. A change to `name` changes them too: there are no pinned
 * versions (FR-17).
 */
export function derivedUsersDeep(
  metrics: readonly DerivedMetric[],
  name: string,
): string[] {
  const found = new Set<string>()
  const pending = [name]
  for (let used = pending.pop(); used !== undefined; used = pending.pop()) {
    for (const user of derivedUsers(metrics, used)) {
      if (!found.has(user)) {
        found.add(user)
        pending.push(user)
      }
    }
  }
  return [...found].sort((a, b) => a.localeCompare(b))
}

/** The results of one evaluation, by sample id. */
export const resultsBySample = (response: EvaluateResponse) =>
  new Map(response.samples.map((s) => [s.sample_id, s]))

/**
 * True if no sample had the inputs of the expression: the metric does not
 * apply to that case execution.
 */
export const allMissing = (response: EvaluateResponse) =>
  response.samples.length > 0 &&
  response.samples.every((s) => s.status === 'missing')
