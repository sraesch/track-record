import { describe, expect, it } from 'vitest'
import {
  hasActiveFilters,
  parseRunFilters,
  runFiltersToParams,
  runFiltersToQuery,
} from './runFilters'

const parse = (query: string) => parseRunFilters(new URLSearchParams(query))

describe('run list filters', () => {
  it('default to valid runs and nothing else', () => {
    const filters = parse('')
    expect(filters).toEqual({
      suite: undefined,
      status: [],
      trigger: undefined,
      branch: undefined,
      device: undefined,
      tags: [],
      validity: 'valid',
      from: undefined,
      to: undefined,
    })
    expect(hasActiveFilters(filters)).toBe(false)
    expect(runFiltersToParams(filters).toString()).toBe('')
  })

  it('survive a round trip through the URL', () => {
    const query =
      'suite=smoke&status=failed&status=aborted&trigger=ci&branch=main' +
      '&device=pixel-8&tag=nightly&tag=baseline&validity=all' +
      '&from=2026-11-01&to=2026-11-20'
    const filters = parse(query)
    expect(filters).toMatchObject({
      suite: 'smoke',
      status: ['failed', 'aborted'],
      trigger: 'ci',
      tags: ['nightly', 'baseline'],
      validity: 'all',
    })
    expect(runFiltersToParams(filters).toString()).toBe(query)
    expect(hasActiveFilters(filters)).toBe(true)
  })

  it('ignore unknown and malformed values', () => {
    const filters = parse(
      'status=done&status=failed&trigger=cron&validity=maybe&from=yesterday&tag=&tag=x&tag=x&suite=%20',
    )
    expect(filters).toMatchObject({
      status: ['failed'],
      trigger: undefined,
      validity: 'valid',
      from: undefined,
      tags: ['x'],
      suite: undefined,
    })
  })

  it('become an API query; the end day is inclusive', () => {
    const query = runFiltersToQuery(
      parse('status=failed&tag=a&device=d&branch=b&from=2026-11-01&to=2026-11-20'),
    )
    expect(query).toMatchObject({
      status: ['failed'],
      tag: ['a'],
      client_device: 'd',
      sut_branch: 'b',
      validity: 'valid',
      started_after: new Date(2026, 10, 1).toISOString(),
      started_before: new Date(2026, 10, 21).toISOString(),
    })
  })

  it('leave empty lists out of the API query', () => {
    expect(runFiltersToQuery(parse(''))).toMatchObject({
      status: undefined,
      tag: undefined,
    })
  })
})
