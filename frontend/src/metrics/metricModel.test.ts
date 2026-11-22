import { describe, expect, it } from 'vitest'
import type { Metric, MetricVersion, Unit } from '../api/client'
import {
  changedFields,
  customUnits,
  filterMetrics,
  filterUnits,
  groupUnits,
  metricsPerUnit,
  metricsViewToParams,
  parseMetricsView,
  toMetricRows,
} from './metricModel'

const version = (
  v: number,
  fields: Partial<MetricVersion> = {},
): MetricVersion => ({
  id: v,
  version: v,
  unit: 'ms',
  value_type: 'float',
  direction: 'lower_is_better',
  description: '',
  created_at: '2026-11-01T00:00:00Z',
  ...fields,
})

const metric = (
  id: number,
  name: string,
  versions: MetricVersion[],
): Metric => ({ id, name, versions })

const unit = (symbol: string, quantity: string, to_base: number): Unit => ({
  symbol,
  quantity,
  to_base,
  name: `${symbol} name`,
})

describe('metrics view in the URL', () => {
  it('reads tab and search, ignoring unknown tabs', () => {
    expect(
      parseMetricsView(new URLSearchParams('tab=units&q=%20time%20')),
    ).toEqual({ tab: 'units', search: 'time' })
    expect(parseMetricsView(new URLSearchParams('tab=x&q=%20'))).toEqual({
      tab: 'metrics',
      search: undefined,
    })
  })

  it('leaves defaults out', () => {
    expect(metricsViewToParams({ tab: 'metrics' }).toString()).toBe('')
    expect(
      metricsViewToParams({ tab: 'units', search: ' MB ' }).toString(),
    ).toBe('tab=units&q=MB')
  })
})

describe('toMetricRows', () => {
  it('sorts by name, versions newest first, skips metrics without versions', () => {
    const rows = toMetricRows([
      metric(1, 'load_time', [version(1), version(3), version(2)]),
      metric(2, 'empty', []),
      metric(3, 'fps', [version(1, { unit: 'fps' })]),
    ])
    expect(rows.map((r) => r.name)).toEqual(['fps', 'load_time'])
    expect(rows[1]!.latest.version).toBe(3)
    expect(rows[1]!.versions.map((v) => v.version)).toEqual([3, 2, 1])
  })
})

describe('filterMetrics', () => {
  const rows = toMetricRows([
    metric(1, 'load_time', [version(1, { description: 'Until loaded' })]),
    metric(2, 'memory', [
      version(1, { unit: 'MB' }),
      version(2, { unit: 'MiB' }),
    ]),
  ])

  it('matches name, description and the unit of any version, ignoring case', () => {
    expect(filterMetrics(rows, 'LOAD').map((r) => r.name)).toEqual([
      'load_time',
    ])
    expect(filterMetrics(rows, 'until').map((r) => r.name)).toEqual([
      'load_time',
    ])
    expect(filterMetrics(rows, 'MB').map((r) => r.name)).toEqual(['memory'])
    expect(filterMetrics(rows, undefined)).toBe(rows)
  })
})

describe('changedFields', () => {
  it('lists what differs from the previous version', () => {
    expect(changedFields(version(1), undefined)).toEqual([])
    expect(
      changedFields(version(2, { unit: 's', description: 'new' }), version(1)),
    ).toEqual(['unit', 'description'])
  })
})

describe('unit catalogue', () => {
  const units = [
    unit('s', 'time', 1),
    unit('ms', 'time', 1e-3),
    unit('min', 'time', 60),
    unit('', 'ratio', 1),
    unit('%', 'ratio', 1e-2),
  ]

  it('groups by quantity with the base unit, units by size', () => {
    const groups = groupUnits(units)
    expect(groups.map((g) => g.quantity)).toEqual(['ratio', 'time'])
    expect(groups[1]!.base?.symbol).toBe('s')
    expect(groups[1]!.units.map((u) => u.symbol)).toEqual(['ms', 's', 'min'])
    expect(groups[0]!.base?.symbol).toBe('')
  })

  it('filters by quantity, symbol or name', () => {
    const groups = groupUnits(units)
    expect(filterUnits(groups, 'TIME')).toEqual([groups[1]])
    expect(filterUnits(groups, 'min')).toEqual([
      { ...groups[1]!, units: [units[2]] },
    ])
    expect(filterUnits(groups, 'nothing')).toEqual([])
  })

  it('finds the metrics per unit and the custom units', () => {
    const rows = toMetricRows([
      metric(1, 'load_time', [version(1, { unit: 's' })]),
      metric(2, 'parse_time', [version(1, { unit: 's' })]),
      // Only the latest version counts.
      metric(3, 'triangles', [
        version(1, { unit: 'ms' }),
        version(2, { unit: 'tris' }),
      ]),
    ])
    expect(metricsPerUnit(rows).get('s')).toEqual(['load_time', 'parse_time'])
    expect(metricsPerUnit(rows).has('ms')).toBe(false)
    expect(customUnits(rows, units)).toEqual([
      { symbol: 'tris', metrics: ['triangles'] },
    ])
  })
})
