import type { Metric, Unit } from '../api/client'

// The metrics view: uploaded metrics, which have one definition that can be
// changed (FR-7, ADR 0018), and the unit catalogue (FR-13a). Its state
// lives in the URL (FR-21):
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

/** The metrics sorted by name. */
export function sortMetrics(metrics: readonly Metric[]): Metric[] {
  return [...metrics].sort((a, b) => a.name.localeCompare(b.name))
}

const matches = (needle: string, ...haystack: string[]) =>
  haystack.some((text) => text.toLowerCase().includes(needle))

/** Metrics whose name, description or unit contain `search`. */
export function filterMetrics(
  metrics: readonly Metric[],
  search: string | undefined,
): readonly Metric[] {
  const needle = search?.trim().toLowerCase()
  if (!needle) return metrics
  return metrics.filter((m) => matches(needle, m.name, m.description, m.unit))
}

/**
 * Whether the backend accepts `name` for a metric: not empty, no spaces
 * around it, no backtick (expressions could not quote it).
 */
export function isValidMetricName(name: string): boolean {
  return name.trim() !== '' && name === name.trim() && !name.includes('`')
}

/** Why a metric cannot be deleted, if it cannot. */
export function deleteBlocker(
  metric: Metric,
): 'results' | 'derived' | undefined {
  if (metric.has_results) return 'results'
  if (metric.used_by.length > 0) return 'derived'
  return undefined
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

/** The names of the metrics that use each unit symbol. */
export function metricsPerUnit(
  metrics: readonly Metric[],
): Map<string, string[]> {
  const perUnit = new Map<string, string[]>()
  for (const metric of metrics) {
    const names = perUnit.get(metric.unit) ?? []
    names.push(metric.name)
    perUnit.set(metric.unit, names)
  }
  return perUnit
}

/**
 * Units used by a metric that are not in the catalogue. They are allowed,
 * but values in them are not converted (FR-13a).
 */
export function customUnits(
  metrics: readonly Metric[],
  units: readonly Unit[],
): { symbol: string; metrics: string[] }[] {
  const known = new Set(units.map((u) => u.symbol))
  return [...metricsPerUnit(metrics).entries()]
    .filter(([symbol]) => !known.has(symbol))
    .map(([symbol, metrics]) => ({ symbol, metrics }))
    .sort((a, b) => a.symbol.localeCompare(b.symbol))
}
