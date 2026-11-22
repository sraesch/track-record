import { describe, expect, it } from 'vitest'
import {
  type AliasRow,
  aliasFiltersToParams,
  filterRows,
  parseAliasFilters,
  parseDimension,
  parseValue,
  resolve,
  sameValue,
  toRows,
} from './aliasModel'

const row = (
  id: number,
  dimension: string,
  source: AliasRow['source'],
  target?: AliasRow['target'],
): AliasRow => ({
  id,
  dimension,
  source,
  target,
  version: 1,
  updatedAt: '2026-11-20T10:00:00Z',
})

describe('alias model', () => {
  it('parses dimensions', () => {
    expect(parseDimension('client_device')).toEqual({ kind: 'device' })
    expect(parseDimension('parameter:size')).toEqual({
      kind: 'parameter',
      name: 'size',
    })
    expect(parseDimension('parameter: ')).toBeUndefined()
    expect(parseDimension('suite')).toBeUndefined()
  })

  it('parses values with their type', () => {
    expect(parseValue(' pixel 8 ', 'string')).toBe('pixel 8')
    expect(parseValue('10', 'string')).toBe('10')
    expect(parseValue('10', 'number')).toBe(10)
    expect(parseValue('-1.5e3', 'number')).toBe(-1500)
    expect(parseValue('0x1f', 'number')).toBeUndefined()
    expect(parseValue('ten', 'number')).toBeUndefined()
    expect(parseValue('true', 'boolean')).toBe(true)
    expect(parseValue('yes', 'boolean')).toBeUndefined()
    expect(parseValue('  ', 'string')).toBeUndefined()
  })

  it('keeps 10 and "10" apart', () => {
    expect(sameValue(10, 10)).toBe(true)
    expect(sameValue(10, '10')).toBe(false)
  })

  it('drops aliases whose values are not scalars', () => {
    const rows = toRows([
      {
        id: 1,
        dimension: 'client_device',
        source: 'a' as never,
        target: 'b' as never,
        version: 2,
        updated_at: '2026-11-20T10:00:00Z',
      },
      {
        id: 2,
        dimension: 'client_device',
        source: 'c' as never,
        target: null as never,
        version: 3,
        updated_at: '2026-11-20T10:00:00Z',
      },
      {
        id: 3,
        dimension: 'client_device',
        source: [1] as never,
        target: 'd' as never,
        version: 1,
        updated_at: '2026-11-20T10:00:00Z',
      },
    ])
    expect(rows.map((r) => [r.source, r.target])).toEqual([
      ['a', 'b'],
      ['c', undefined],
    ])
  })

  it('follows chains as the backend does', () => {
    const rows = [
      row(1, 'client_device', 'a', 'b'),
      row(2, 'client_device', 'b', 'c'),
      row(3, 'client_device', 'x', 'y'),
      row(4, 'client_device', 'y', 'x'), // cycle
      row(5, 'client_device', 'c', undefined), // removed
      row(6, 'parameter:size', 'b', 'z'), // another dimension
    ]
    expect(resolve(rows, 'client_device', 'a')).toBe('c')
    expect(resolve(rows, 'client_device', 'c')).toBe('c')
    expect(resolve(rows, 'client_device', 'x')).toBe('y')
  })

  it('reads and writes the filters of the URL', () => {
    const filters = parseAliasFilters(
      new URLSearchParams('dimension=parameter%3Asize&q=+fast+&removed=1'),
    )
    expect(filters).toEqual({
      dimension: 'parameter:size',
      search: 'fast',
      removed: true,
    })
    expect(aliasFiltersToParams(filters).toString()).toBe(
      'dimension=parameter%3Asize&q=fast&removed=1',
    )
    expect(
      parseAliasFilters(new URLSearchParams('dimension=bogus&removed=yes')),
    ).toEqual({ dimension: undefined, search: undefined, removed: false })
  })

  it('filters rows', () => {
    const rows = [
      row(1, 'client_device', 'pixel8', 'pixel-8'),
      row(2, 'parameter:mode', 'fast', 'quick'),
      row(3, 'parameter:mode', 'slow', undefined),
    ]
    const ids = (filters: Parameters<typeof filterRows>[1]) =>
      filterRows(rows, filters).map((r) => r.id)
    expect(ids({ removed: false })).toEqual([1, 2])
    expect(ids({ removed: true })).toEqual([1, 2, 3])
    expect(ids({ removed: true, dimension: 'parameter:mode' })).toEqual([2, 3])
    expect(ids({ removed: false, search: 'PIXEL' })).toEqual([1])
    expect(ids({ removed: false, search: 'mode' })).toEqual([2])
  })
})
