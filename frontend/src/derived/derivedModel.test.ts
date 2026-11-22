import { describe, expect, it } from 'vitest'
import type { DerivedMetric } from '../api/client'
import {
  allMissing,
  derivedUsers,
  derivedUsersDeep,
  derivedViewToParams,
  filterDerived,
  parseDerivedView,
  sortDerived,
} from './derivedModel'

const metric = (
  name: string,
  overrides: Partial<DerivedMetric> = {},
): DerivedMetric => ({
  id: name.length,
  name,
  expression: 'bytes / load_time',
  unit: 'MB/s',
  description: '',
  created_at: '2026-11-01T08:00:00Z',
  updated_at: '2026-11-02T08:00:00Z',
  uses: [],
  ...overrides,
})

const metrics: DerivedMetric[] = [
  metric('throughput', { unit: 'Mbit/s' }),
  metric('efficiency', {
    expression: 'derived.throughput / power',
    description: 'Per watt',
    uses: ['throughput'],
  }),
  metric('fresh', {
    expression: 'derived.throughput * 2',
    uses: ['throughput'],
  }),
  metric('budget', {
    expression: 'derived.efficiency * 60',
    uses: ['efficiency'],
  }),
]

describe('derived metrics view', () => {
  it('reads and writes the URL', () => {
    const view = parseDerivedView(
      new URLSearchParams('q=+fps+&name=throughput&edit=change&run=r1&case=c1'),
    )
    expect(view).toEqual({
      search: 'fps',
      name: 'throughput',
      edit: 'change',
      run: 'r1',
      caseExecution: 'c1',
    })
    expect(derivedViewToParams(view).toString()).toBe(
      'q=fps&name=throughput&edit=change&run=r1&case=c1',
    )
  })

  it('ignores what does not make sense', () => {
    expect(parseDerivedView(new URLSearchParams('edit=change&case='))).toEqual(
      {},
    )
    expect(parseDerivedView(new URLSearchParams('name=a&edit=other'))).toEqual({
      name: 'a',
    })
    expect(parseDerivedView(new URLSearchParams('edit=new'))).toEqual({
      edit: 'new',
    })
    // Links from when derived metrics had versions show the metric.
    expect(
      parseDerivedView(new URLSearchParams('name=a&version=1&edit=version')),
    ).toEqual({ name: 'a' })
  })
})

describe('derived metric list', () => {
  const sorted = sortDerived(metrics)

  it('sorts metrics by name', () => {
    expect(sorted.map((m) => m.name)).toEqual([
      'budget',
      'efficiency',
      'fresh',
      'throughput',
    ])
  })

  it('searches names, expressions, units and descriptions', () => {
    const names = (search: string) =>
      filterDerived(sorted, search).map((m) => m.name)
    expect(names('')).toHaveLength(4)
    expect(names('THROUGH')).toEqual(['efficiency', 'fresh', 'throughput'])
    expect(names('mbit')).toEqual(['throughput'])
    expect(names('per watt')).toEqual(['efficiency'])
  })

  it('lists the metrics built on one, directly or through others', () => {
    expect(derivedUsers(metrics, 'throughput')).toEqual(['efficiency', 'fresh'])
    expect(derivedUsersDeep(metrics, 'throughput')).toEqual([
      'budget',
      'efficiency',
      'fresh',
    ])
    expect(derivedUsers(metrics, 'fresh')).toEqual([])
    expect(derivedUsersDeep(metrics, 'fresh')).toEqual([])
  })
})

describe('allMissing', () => {
  const response = (statuses: ('value' | 'missing')[]) => ({
    expression: 'x',
    kind: 'scalar',
    unit: '',
    samples: statuses.map((status, i) => ({
      sample_id: `s${i}`,
      repetition: i,
      is_warmup: false,
      status,
    })),
  })

  it('is true only if every sample lacks the inputs', () => {
    expect(allMissing(response(['missing', 'missing']))).toBe(true)
    expect(allMissing(response(['missing', 'value']))).toBe(false)
    expect(allMissing(response([]))).toBe(false)
  })
})
