import { describe, expect, it } from 'vitest'
import {
  cleanParam,
  countMoreFilters,
  hasActiveFilters,
  parseRunFilters,
  runFiltersToParams,
  runFiltersToQuery,
  runListHref,
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
      commit: undefined,
      sutTag: undefined,
      buildConfig: undefined,
      environment: undefined,
      scenario: undefined,
      asset: undefined,
      metric: undefined,
      params: [],
    })
    expect(hasActiveFilters(filters)).toBe(false)
    expect(countMoreFilters(filters)).toBe(0)
    expect(runFiltersToParams(filters).toString()).toBe('')
  })

  it('survive a round trip through the URL', () => {
    const query =
      'suite=smoke&status=failed&status=aborted&trigger=ci&branch=main' +
      '&device=pixel-8&tag=nightly&tag=baseline&validity=all' +
      '&from=2026-11-01&to=2026-11-20&commit=168fed94&sutTag=v1.5.0' +
      '&build=release&env=3&scenario=load-model&asset=city&metric=fps' +
      '&param=size%3D10&param=mode%3D%2210%22'
    const filters = parse(query)
    expect(filters).toMatchObject({
      suite: 'smoke',
      status: ['failed', 'aborted'],
      trigger: 'ci',
      tags: ['nightly', 'baseline'],
      validity: 'all',
      environment: 3,
      params: ['size=10', 'mode="10"'],
    })
    expect(runFiltersToParams(filters).toString()).toBe(query)
    expect(hasActiveFilters(filters)).toBe(true)
    expect(countMoreFilters(filters)).toBe(8)
  })

  it('keep only well-formed parameter filters, trimmed and each once', () => {
    expect(cleanParam(' size = 10 ')).toBe('size=10')
    expect(cleanParam('label=')).toBe('label=')
    expect(cleanParam('a=b=c')).toBe('a=b=c')
    expect(cleanParam('size')).toBeUndefined()
    expect(cleanParam(' =10')).toBeUndefined()
    expect(
      parse('param=size%3D10&param=size+%3D+10&param=oops&param=%3D1').params,
    ).toEqual(['size=10'])
  })

  it('take only positive whole numbers as environment', () => {
    for (const env of ['0', '-1', '1.5', 'abc', '99999999999999999999']) {
      expect(parse(`env=${env}`).environment).toBeUndefined()
    }
    expect(parse('env=42').environment).toBe(42)
  })

  it('make links to the run list', () => {
    expect(runListHref({})).toBe('/runs')
    expect(runListHref({ environment: 7 })).toBe('/runs?env=7')
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

  it('ignore days that do not exist or are implausibly old', () => {
    expect(parse('from=2026-02-31&to=0202-11-01')).toMatchObject({
      from: undefined,
      to: undefined,
    })
    expect(parse('from=2024-02-29').from).toBe('2024-02-29')
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
    expect(
      runFiltersToQuery(
        parse(
          'commit=c&sutTag=t&build=b&env=3&scenario=s&asset=a&metric=m' +
            '&param=x%3D1',
        ),
      ),
    ).toMatchObject({
      sut_commit: 'c',
      sut_tag: 't',
      sut_build_config: 'b',
      shared_environment_id: 3,
      scenario: 's',
      asset: 'a',
      metric: 'm',
      param: ['x=1'],
    })
  })

  it('leave empty lists out of the API query', () => {
    expect(runFiltersToQuery(parse(''))).toMatchObject({
      status: undefined,
      tag: undefined,
      param: undefined,
    })
  })
})
