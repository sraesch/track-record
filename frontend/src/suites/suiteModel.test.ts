import { describe, expect, it } from 'vitest'
import type { TestCaseSummary, VersionedDefinition } from '../api/client'
import {
  casesOfVersion,
  casesPerVersion,
  definitionEntries,
  filterDefinitions,
  parseSuitesView,
  selectedRow,
  sortDefinitions,
  type SuitesView,
  suitesViewToParams,
  versionRows,
} from './suiteModel'

const json = (value: object) => value as Record<string, never>

const parse = (query: string) => parseSuitesView(new URLSearchParams(query))

const definition = (
  name: string,
  ...definitions: object[]
): VersionedDefinition => ({
  id: name.length,
  name,
  versions: definitions.map((d, index) => ({
    id: 10 + index,
    version: index + 1,
    definition: json(d),
    created_at: `2026-11-0${index + 1}T08:00:00Z`,
  })),
})

const testCase = (
  id: number,
  version: number,
  tags: string[] = [],
): TestCaseSummary => ({
  id,
  scenario: 'orbit',
  scenario_version: version,
  scenario_version_id: version,
  parameters: json({}),
  resolved_parameters: json({}),
  parameter_units: json({}),
  assets: [],
  tags,
})

describe('suites view in the URL', () => {
  it('shows the suites when the URL says nothing', () => {
    expect(parse('')).toEqual({ tab: 'suites' })
    expect(suitesViewToParams({ tab: 'suites' }).toString()).toBe('')
  })

  it('reads and writes a scenario version with a test case', () => {
    const view: SuitesView = {
      tab: 'scenarios',
      search: 'orb',
      name: 'orbit',
      version: 2,
      testCase: 17,
      tag: 'flaky',
    }
    const query = suitesViewToParams(view).toString()
    expect(query).toBe('tab=scenarios&q=orb&name=orbit&version=2&case=17&tag=flaky')
    expect(parse(query)).toEqual(view)
  })

  it('ignores what does not fit', () => {
    // A version needs its definition; only scenarios have test cases.
    expect(parse('version=2&case=3')).toEqual({ tab: 'suites' })
    expect(parse('name=nightly&version=2&case=3&tag=x')).toEqual({
      tab: 'suites',
      name: 'nightly',
      version: 2,
    })
    expect(parse('tab=scenarios&name=orbit&version=0&case=x')).toEqual({
      tab: 'scenarios',
      name: 'orbit',
    })
    expect(parse('tab=nope&q=%20')).toEqual({ tab: 'suites' })
  })
})

describe('definitions', () => {
  it('sorts by name and filters by it', () => {
    const sorted = sortDefinitions([definition('smoke', {}), definition('Nightly', {})])
    expect(sorted.map((d) => d.name)).toEqual(['Nightly', 'smoke'])
    expect(filterDefinitions(sorted, ' NIGHT ').map((d) => d.name)).toEqual([
      'Nightly',
    ])
    expect(filterDefinitions(sorted, undefined)).toBe(sorted)
  })

  it('lists the details of a first version without changes', () => {
    expect(definitionEntries({ samples: 3, camera: { path: 'orbit' } })).toEqual([
      { key: 'samples', value: 3 },
      { key: 'camera.path', value: 'orbit' },
    ])
  })

  it('marks what differs from the version before', () => {
    const before = {
      camera: { path: 'orbit', speed: 1 },
      steps: ['open', 'load'],
      cache: 'cold',
      samples: '3',
    }
    const after = {
      camera: { path: 'orbit-v2', speed: 1 },
      steps: ['open', 'load'],
      // Another JSON type is a change too.
      samples: 3,
      timeout_s: 10,
    }
    expect(definitionEntries(after, before)).toEqual([
      { key: 'camera.path', value: 'orbit-v2', change: 'changed', previous: 'orbit' },
      { key: 'camera.speed', value: 1 },
      { key: 'steps', value: ['open', 'load'] },
      { key: 'samples', value: 3, change: 'changed', previous: '3' },
      { key: 'timeout_s', value: 10, change: 'added' },
      { key: 'cache', value: 'cold', change: 'removed' },
    ])
  })

  it('lists the versions newest first with their changes', () => {
    const rows = versionRows(
      definition('orbit', { path: 'a', cache: 'cold' }, { path: 'b', fov: 60 }),
    )
    expect(rows.map((row) => row.version.version)).toEqual([2, 1])
    expect(rows[0]?.previous?.version).toBe(1)
    expect(rows[0]?.changes).toEqual({
      changed: ['path'],
      added: ['fov'],
      removed: ['cache'],
    })
    expect(rows[1]?.previous).toBeUndefined()
    expect(rows[1]?.changes).toEqual({ changed: [], added: [], removed: [] })
  })

  it('selects the latest version unless the URL names one', () => {
    const rows = versionRows(definition('orbit', {}, { a: 1 }, { a: 2 }))
    expect(selectedRow(rows, undefined)?.version.version).toBe(3)
    expect(selectedRow(rows, 1)?.version.version).toBe(1)
    expect(selectedRow(rows, 9)).toBeUndefined()
  })
})

describe('test cases of a scenario', () => {
  const cases = [
    testCase(1, 1),
    testCase(2, 2, ['flaky']),
    testCase(3, 2),
    testCase(4, 2, ['flaky', 'heavy']),
  ]

  it('counts them per version', () => {
    expect([...casesPerVersion(cases)]).toEqual([
      [1, 1],
      [2, 3],
    ])
  })

  it('keeps those of one version, optionally with a tag', () => {
    expect(casesOfVersion(cases, 2).map((tc) => tc.id)).toEqual([2, 3, 4])
    expect(casesOfVersion(cases, 2, 'flaky').map((tc) => tc.id)).toEqual([2, 4])
    expect(casesOfVersion(cases, 3)).toEqual([])
  })
})
