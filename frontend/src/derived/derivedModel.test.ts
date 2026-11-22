import { describe, expect, it } from 'vitest'
import type { DerivedMetric, DerivedMetricVersion } from '../api/client'
import {
  allMissing,
  derivedUsers,
  derivedViewToParams,
  filterDerived,
  parseDerivedView,
  toDerivedRows,
} from './derivedModel'

const version = (
  name: string,
  n: number,
  overrides: Partial<DerivedMetricVersion> = {},
): DerivedMetricVersion => ({
  id: n * 100 + name.length,
  name,
  version: n,
  expression: 'bytes / load_time',
  unit: 'MB/s',
  description: '',
  created_at: '2026-11-01T08:00:00Z',
  uses: [],
  ...overrides,
})

const metrics: DerivedMetric[] = [
  {
    id: 1,
    name: 'throughput',
    versions: [version('throughput', 1), version('throughput', 2, { unit: 'Mbit/s' })],
  },
  {
    id: 2,
    name: 'efficiency',
    versions: [
      version('efficiency', 1, {
        expression: 'derived.throughput / power',
        description: 'Per watt',
        uses: [{ name: 'throughput', version: 1, version_id: 105 }],
      }),
    ],
  },
  {
    id: 3,
    name: 'fresh',
    versions: [
      version('fresh', 1, {
        expression: 'derived.throughput * 2',
        uses: [{ name: 'throughput', version: 2, version_id: 210 }],
      }),
    ],
  },
  { id: 4, name: 'empty', versions: [] },
]

describe('derived metrics view', () => {
  it('reads and writes the URL', () => {
    const view = parseDerivedView(
      new URLSearchParams(
        'q=+fps+&name=throughput&version=2&edit=version&run=r1&case=c1',
      ),
    )
    expect(view).toEqual({
      search: 'fps',
      name: 'throughput',
      version: 2,
      edit: 'version',
      run: 'r1',
      caseExecution: 'c1',
    })
    expect(derivedViewToParams(view).toString()).toBe(
      'q=fps&name=throughput&version=2&edit=version&run=r1&case=c1',
    )
  })

  it('ignores what does not make sense', () => {
    expect(
      parseDerivedView(new URLSearchParams('version=0&edit=version&case=')),
    ).toEqual({})
    expect(
      parseDerivedView(new URLSearchParams('name=a&version=x&edit=other')),
    ).toEqual({ name: 'a' })
    expect(parseDerivedView(new URLSearchParams('edit=new'))).toEqual({
      edit: 'new',
    })
  })
})

describe('derived metric rows', () => {
  const rows = toDerivedRows(metrics)

  it('sorts metrics by name and versions newest first', () => {
    expect(rows.map((r) => r.name)).toEqual(['efficiency', 'fresh', 'throughput'])
    expect(rows[2]?.latest.version).toBe(2)
    expect(rows[2]?.versions.map((v) => v.version)).toEqual([2, 1])
  })

  it('searches names, expressions, units and descriptions', () => {
    const names = (search: string) =>
      filterDerived(rows, search).map((r) => r.name)
    expect(names('')).toHaveLength(3)
    expect(names('THROUGH')).toEqual(['efficiency', 'fresh', 'throughput'])
    expect(names('mbit')).toEqual(['throughput'])
    expect(names('per watt')).toEqual(['efficiency'])
  })

  it('lists the metrics using one, marking outdated pins', () => {
    expect(derivedUsers(rows, 'throughput')).toEqual([
      { name: 'efficiency', version: 1, pinned: 1, outdated: true },
      { name: 'fresh', version: 1, pinned: 2, outdated: false },
    ])
    expect(derivedUsers(rows, 'fresh')).toEqual([])
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
