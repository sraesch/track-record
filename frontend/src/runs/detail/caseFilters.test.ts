import { describe, expect, it } from 'vitest'
import type { CaseExecutionSummary } from '../../api/client'
import {
  type CaseFilters,
  caseFiltersToParams,
  filterCaseExecutions,
  hasActiveCaseFilters,
  parseCaseFilters,
} from './caseFilters'
import { flattenJson } from './json'

const json = (value: object) => value as Record<string, never>

function ce(
  id: string,
  scenario: string,
  device: { name: string; resolved: string },
  parameters: object,
  resolved: object = parameters,
): CaseExecutionSummary {
  return {
    id,
    run_id: 'r1',
    test_case: {
      id: 1,
      scenario,
      scenario_version: 1,
      scenario_version_id: 1,
      parameters: json(parameters),
      resolved_parameters: json(resolved),
      parameter_units: json({}),
      assets: [{ id: 1, name: 'city-block', version: 'v3' }],
      tags: ['smoke'],
    },
    client_device: { id: 1, name: device.name, resolved_name: device.resolved },
    device_environment_id: 1,
    created_at: '2026-11-20T03:00:00Z',
    sample_count: 3,
  }
}

const pixel = { name: 'pixel8-lab1', resolved: 'pixel-8-lab-1' }
const iphone = { name: 'iphone-15', resolved: 'iphone-15' }
const all = [
  ce(
    'a',
    'orbit',
    pixel,
    { pipeline: 'meshopt-v1', lod: true },
    { pipeline: 'meshopt', lod: true },
  ),
  ce('b', 'orbit', iphone, { pipeline: 'draco', bandwidth: 20 }),
  ce('c', 'zoom', iphone, { pipeline: 'draco', bandwidth: 100 }),
]
const ids = (list: CaseExecutionSummary[]) => list.map((c) => c.id)

describe('case execution filters', () => {
  it('round-trips through the URL, leaving out defaults', () => {
    const query = 'scenario=orbit&device=pixel-8-lab-1&search=meshopt+lod&superseded=true'
    const filters = parseCaseFilters(new URLSearchParams(query))
    expect(filters).toEqual({
      scenario: 'orbit',
      device: 'pixel-8-lab-1',
      search: 'meshopt lod',
      superseded: true,
    })
    expect(caseFiltersToParams(filters).toString()).toBe(query)
    const ignored = new URLSearchParams('superseded=yes&scenario=+')
    expect(caseFiltersToParams(parseCaseFilters(ignored)).toString()).toBe('')
  })

  it('counts only filters that narrow the list as active', () => {
    expect(hasActiveCaseFilters({ superseded: true })).toBe(false)
    expect(hasActiveCaseFilters({ search: '  ', superseded: false })).toBe(false)
    expect(hasActiveCaseFilters({ device: 'x', superseded: false })).toBe(true)
  })

  it('matches scenario and device exactly, the device after aliases', () => {
    const match = (filters: Partial<CaseFilters>) =>
      ids(filterCaseExecutions(all, { superseded: false, ...filters }))
    expect(match({ scenario: 'orbit' })).toEqual(['a', 'b'])
    expect(match({ device: 'pixel-8-lab-1' })).toEqual(['a'])
    expect(match({ device: 'pixel8-lab1' })).toEqual([])
  })

  it('searches every word in scenario, parameters, assets, tags and devices', () => {
    const search = (text: string) =>
      ids(filterCaseExecutions(all, { search: text, superseded: false }))
    expect(search('ZOOM')).toEqual(['c'])
    expect(search('bandwidth=20')).toEqual(['b'])
    expect(search('draco iphone')).toEqual(['b', 'c'])
    // Values after aliases and as uploaded, devices both ways.
    expect(search('pipeline=meshopt')).toEqual(['a'])
    expect(search('meshopt-v1')).toEqual(['a'])
    expect(search('pixel8')).toEqual(['a'])
    expect(search('lod=true')).toEqual(['a'])
    expect(search('city-block v3 smoke')).toEqual(['a', 'b', 'c'])
    expect(search('draco orbit zoom')).toEqual([])
  })
})

describe('flattenJson', () => {
  it('joins nested keys with dots and keeps leaves as they are', () => {
    expect(
      flattenJson({
        backend: { name: 'hub', limits: { cpu: 4 } },
        regions: ['eu', 'us'],
        gpu: {},
        debug: false,
        note: null,
      }),
    ).toEqual([
      ['backend.name', 'hub'],
      ['backend.limits.cpu', 4],
      ['regions', ['eu', 'us']],
      ['gpu', {}],
      ['debug', false],
      ['note', null],
    ])
  })
})
