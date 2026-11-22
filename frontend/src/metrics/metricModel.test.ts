import { describe, expect, it } from 'vitest'
import type { Metric, Unit } from '../api/client'
import {
  customUnits,
  deleteBlocker,
  filterMetrics,
  filterUnits,
  groupUnits,
  isValidMetricName,
  metricsPerUnit,
  metricsViewToParams,
  parseMetricsView,
  sortMetrics,
} from './metricModel'

const metric = (
  id: number,
  name: string,
  fields: Partial<Metric> = {},
): Metric => ({
  id,
  name,
  unit: 'ms',
  value_type: 'float',
  direction: 'lower_is_better',
  description: '',
  created_at: '2026-11-01T00:00:00Z',
  updated_at: '2026-11-01T00:00:00Z',
  has_results: true,
  used_by: [],
  ...fields,
})

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

describe('sortMetrics', () => {
  it('sorts by name', () => {
    const metrics = [metric(1, 'load_time'), metric(3, 'fps')]
    expect(sortMetrics(metrics).map((m) => m.name)).toEqual([
      'fps',
      'load_time',
    ])
    expect(metrics[0]!.name).toBe('load_time')
  })
})

describe('filterMetrics', () => {
  const metrics = [
    metric(1, 'load_time', { description: 'Until loaded' }),
    metric(2, 'memory', { unit: 'MiB' }),
  ]

  it('matches name, description and unit, ignoring case', () => {
    expect(filterMetrics(metrics, 'LOAD').map((m) => m.name)).toEqual([
      'load_time',
    ])
    expect(filterMetrics(metrics, 'until').map((m) => m.name)).toEqual([
      'load_time',
    ])
    expect(filterMetrics(metrics, 'mib').map((m) => m.name)).toEqual(['memory'])
    expect(filterMetrics(metrics, undefined)).toBe(metrics)
  })
})

describe('isValidMetricName', () => {
  it('rejects names that expressions could not refer to', () => {
    expect(isValidMetricName('load time (cold)')).toBe(true)
    expect(isValidMetricName('')).toBe(false)
    expect(isValidMetricName(' load_time')).toBe(false)
    expect(isValidMetricName('load`time')).toBe(false)
  })
})

describe('deleteBlocker', () => {
  it('says why a metric cannot be deleted', () => {
    expect(deleteBlocker(metric(1, 'a'))).toBe('results')
    expect(
      deleteBlocker(metric(1, 'a', { has_results: false, used_by: ['d'] })),
    ).toBe('derived')
    expect(deleteBlocker(metric(1, 'a', { has_results: false }))).toBe(
      undefined,
    )
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
    const rows = [
      metric(1, 'load_time', { unit: 's' }),
      metric(2, 'parse_time', { unit: 's' }),
      metric(3, 'triangles', { unit: 'tris' }),
    ]
    expect(metricsPerUnit(rows).get('s')).toEqual(['load_time', 'parse_time'])
    expect(metricsPerUnit(rows).has('ms')).toBe(false)
    expect(customUnits(rows, units)).toEqual([
      { symbol: 'tris', metrics: ['triangles'] },
    ])
  })
})
