import type {
  DerivedMetric,
  DerivedMetricVersion,
  EvaluateResponse,
} from '../api/client'

// The derived metrics view (FR-17). Its state lives in the URL (FR-21):
//
//   /derived-metrics?q=fps                       list filtered by the search
//   /derived-metrics?name=throughput             latest version selected
//   /derived-metrics?name=throughput&version=1   an older version selected
//   /derived-metrics?edit=new                    editor for a new metric
//   /derived-metrics?name=throughput&edit=version  editor for a new version
//   ...&run=<id>&case=<id>                       the editor's preview case

export type DerivedEdit = 'new' | 'version'

export type DerivedView = {
  /** Text filter of the list, trimmed; undefined when empty. */
  search?: string
  /** The selected derived metric. */
  name?: string
  /** The selected version; undefined means the latest. */
  version?: number
  /** The editor in the right panel. */
  edit?: DerivedEdit
  /** Run and case execution the editor previews the expression on. */
  run?: string
  caseExecution?: string
}

const positiveInt = (value: string | null) => {
  if (!value || !/^\d+$/.test(value)) return undefined
  const n = Number(value)
  return n > 0 ? n : undefined
}

export function parseDerivedView(params: URLSearchParams): DerivedView {
  const name = params.get('name') || undefined
  const edit = params.get('edit')
  return {
    search: params.get('q')?.trim() || undefined,
    name,
    version: name ? positiveInt(params.get('version')) : undefined,
    // A new version needs the metric it is a version of.
    edit:
      edit === 'new' ? 'new' : edit === 'version' && name ? 'version' : undefined,
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
  if (view.name && view.version) params.set('version', String(view.version))
  if (view.edit) params.set('edit', view.edit)
  if (view.run) params.set('run', view.run)
  if (view.caseExecution) params.set('case', view.caseExecution)
  return params
}

/** A derived metric with its versions newest first. */
export type DerivedRow = {
  id: number
  name: string
  latest: DerivedMetricVersion
  /** All versions, newest first; the first one is `latest`. */
  versions: DerivedMetricVersion[]
}

/** One row per derived metric with at least one version, sorted by name. */
export function toDerivedRows(metrics: readonly DerivedMetric[]): DerivedRow[] {
  return metrics
    .flatMap((metric) => {
      const versions = [...metric.versions].sort(
        (a, b) => b.version - a.version,
      )
      const latest = versions[0]
      return latest
        ? [{ id: metric.id, name: metric.name, latest, versions }]
        : []
    })
    .sort((a, b) => a.name.localeCompare(b.name))
}

/** Derived metrics whose name, expression, unit or description contain `search`. */
export function filterDerived(
  rows: readonly DerivedRow[],
  search: string | undefined,
): readonly DerivedRow[] {
  const needle = search?.trim().toLowerCase()
  if (!needle) return rows
  return rows.filter(({ name, latest }) =>
    [name, latest.expression, latest.unit, latest.description].some((text) =>
      text.toLowerCase().includes(needle),
    ),
  )
}

/** A derived metric whose latest version uses another one. */
export type DerivedUser = {
  name: string
  /** Its latest version. */
  version: number
  /** The version of the used metric it pins (FR-17). */
  pinned: number
  /** True if that is not the used metric's latest version. */
  outdated: boolean
}

/**
 * The derived metrics whose latest version uses `name`, with the version
 * they pin. A new version of `name` only takes effect through a new
 * version of the metrics built on it (FR-17), so outdated pins are marked.
 */
export function derivedUsers(
  rows: readonly DerivedRow[],
  name: string,
): DerivedUser[] {
  const latest = rows.find((row) => row.name === name)?.latest.version
  return rows.flatMap((row) => {
    const used = row.latest.uses.find((u) => u.name === name)
    return used
      ? [
          {
            name: row.name,
            version: row.latest.version,
            pinned: used.version,
            outdated: latest !== undefined && used.version < latest,
          },
        ]
      : []
  })
}

/** The latest version of every derived metric, by name. */
export function latestVersions(
  rows: readonly DerivedRow[],
): ReadonlyMap<string, number> {
  return new Map(rows.map((row) => [row.name, row.latest.version]))
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
