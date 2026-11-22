import type { Metric, MetricVersion, Unit } from '../api/client'

// The metrics view: metric definitions (versioned, FR-7) and the unit
// catalogue (FR-13a). Its state lives in the URL (FR-21):
//
//   /metrics?q=load              metrics whose name, description or unit match
//   /metrics?tab=units&q=time    the unit catalogue, filtered

export type MetricsTab = 'metrics' | 'units'

export type MetricsView = {
  tab: MetricsTab
  /** Text filter, trimmed; undefined when empty. */
  search?: string
}

export function parseMetricsView(params: URLSearchParams): MetricsView {
  return {
    tab: params.get('tab') === 'units' ? 'units' : 'metrics',
    search: params.get('q')?.trim() || undefined,
  }
}

/** The view as a URL query; defaults are left out. */
export function metricsViewToParams(view: MetricsView): URLSearchParams {
  const params = new URLSearchParams()
  if (view.tab !== 'metrics') params.set('tab', view.tab)
  if (view.search?.trim()) params.set('q', view.search.trim())
  return params
}

/** A metric as the table shows it: its latest version and the older ones. */
export type MetricRow = {
  id: number
  name: string
  latest: MetricVersion
  /** All versions, newest first; the first one is `latest`. */
  versions: MetricVersion[]
}

/** One row per metric with at least one version, sorted by name. */
export function toMetricRows(metrics: readonly Metric[]): MetricRow[] {
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

const matches = (needle: string, ...haystack: string[]) =>
  haystack.some((text) => text.toLowerCase().includes(needle))

/** Metrics whose name, description or unit (of any version) contain `search`. */
export function filterMetrics(
  rows: readonly MetricRow[],
  search: string | undefined,
): readonly MetricRow[] {
  const needle = search?.trim().toLowerCase()
  if (!needle) return rows
  return rows.filter((row) =>
    row.versions.some((v) => matches(needle, row.name, v.description, v.unit)),
  )
}

/** The fields that differ from the previous version, to mark them. */
export type ChangedField = 'unit' | 'value_type' | 'direction' | 'description'

export function changedFields(
  version: MetricVersion,
  previous: MetricVersion | undefined,
): ChangedField[] {
  if (!previous) return []
  return (['unit', 'value_type', 'direction', 'description'] as const).filter(
    (field) => version[field] !== previous[field],
  )
}

/** The units of one quantity, base unit first, then by size. */
export type QuantityGroup = {
  quantity: string
  /** The unit with `to_base` 1, if the catalogue has one. */
  base?: Unit
  units: Unit[]
}

/** The catalogue grouped by quantity, quantities sorted by name. */
export function groupUnits(units: readonly Unit[]): QuantityGroup[] {
  const groups = new Map<string, Unit[]>()
  for (const unit of units) {
    let group = groups.get(unit.quantity)
    if (!group) groups.set(unit.quantity, (group = []))
    group.push(unit)
  }
  return [...groups.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([quantity, members]) => ({
      quantity,
      base: members.find((u) => u.to_base === 1),
      units: [...members].sort(
        (a, b) => a.to_base - b.to_base || a.symbol.localeCompare(b.symbol),
      ),
    }))
}

/** Quantities and units whose quantity, symbol or name contain `search`. */
export function filterUnits(
  groups: readonly QuantityGroup[],
  search: string | undefined,
): QuantityGroup[] {
  const needle = search?.trim().toLowerCase()
  if (!needle) return [...groups]
  return groups.flatMap((group) => {
    if (matches(needle, group.quantity)) return [group]
    const units = group.units.filter((u) => matches(needle, u.symbol, u.name))
    return units.length > 0 ? [{ ...group, units }] : []
  })
}

/** How many metrics use each unit symbol in their latest version. */
export function metricsPerUnit(
  rows: readonly MetricRow[],
): Map<string, string[]> {
  const perUnit = new Map<string, string[]>()
  for (const row of rows) {
    const names = perUnit.get(row.latest.unit) ?? []
    names.push(row.name)
    perUnit.set(row.latest.unit, names)
  }
  return perUnit
}

/**
 * Units used by a metric (latest version) that are not in the catalogue.
 * They are allowed, but values in them are not converted (FR-13a).
 */
export function customUnits(
  rows: readonly MetricRow[],
  units: readonly Unit[],
): { symbol: string; metrics: string[] }[] {
  const known = new Set(units.map((u) => u.symbol))
  return [...metricsPerUnit(rows).entries()]
    .filter(([symbol]) => !known.has(symbol))
    .map(([symbol, metrics]) => ({ symbol, metrics }))
    .sort((a, b) => a.symbol.localeCompare(b.symbol))
}
