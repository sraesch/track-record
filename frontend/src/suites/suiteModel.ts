import type {
  DefinitionVersion,
  TestCaseSummary,
  VersionedDefinition,
} from '../api/client'
import { flattenJson, jsonObject } from '../runs/detail/json'

// The suites & scenarios view: the definitions the harness uploads with
// its results, with all their versions (FR-7), and the test cases of a
// scenario version. Nothing is created or changed here except the tags and
// annotations of a test case. Its state lives in the URL (FR-21):
//
//   /suites?q=night                       suites whose name matches
//   /suites?name=nightly                  a suite, its latest version
//   /suites?name=nightly&version=2        one of its versions
//   /suites?tab=scenarios&name=orbit&version=2&case=17
//                                         a scenario version and one of its
//                                         test cases
//   ...&tag=flaky                         only its test cases with this tag

export type SuitesTab = 'suites' | 'scenarios'

export type SuitesView = {
  tab: SuitesTab
  /** Text filter of the list, trimmed; undefined when empty. */
  search?: string
  /** The selected suite or scenario. */
  name?: string
  /** The selected version; the latest one when not set. */
  version?: number
  /** Scenarios: the selected test case. */
  testCase?: number
  /** Scenarios: only test cases with this tag. */
  tag?: string
}

/** A positive whole number, e.g. a version or a database ID. */
function positive(value: string | null): number | undefined {
  if (!value || !/^\d+$/.test(value)) return undefined
  const n = Number(value)
  return Number.isSafeInteger(n) && n > 0 ? n : undefined
}

/** Reads the view from a URL query; unknown or malformed values are ignored. */
export function parseSuitesView(params: URLSearchParams): SuitesView {
  const tab = params.get('tab') === 'scenarios' ? 'scenarios' : 'suites'
  const name = params.get('name') || undefined
  // Versions belong to a definition, test cases and their tags to a scenario.
  const scenario = tab === 'scenarios' && name !== undefined
  return {
    tab,
    search: params.get('q')?.trim() || undefined,
    name,
    version: name ? positive(params.get('version')) : undefined,
    testCase: scenario ? positive(params.get('case')) : undefined,
    tag: scenario ? params.get('tag')?.trim() || undefined : undefined,
  }
}

/** The view as a URL query; defaults are left out. */
export function suitesViewToParams(view: SuitesView): URLSearchParams {
  const params = new URLSearchParams()
  if (view.tab !== 'suites') params.set('tab', view.tab)
  if (view.search?.trim()) params.set('q', view.search.trim())
  if (view.name) params.set('name', view.name)
  if (view.version) params.set('version', String(view.version))
  if (view.testCase) params.set('case', String(view.testCase))
  if (view.tag) params.set('tag', view.tag)
  return params
}

/** The definitions sorted by name. */
export function sortDefinitions(
  definitions: readonly VersionedDefinition[],
): VersionedDefinition[] {
  return [...definitions].sort((a, b) => a.name.localeCompare(b.name))
}

/** Definitions whose name contains `search`. */
export function filterDefinitions(
  definitions: readonly VersionedDefinition[],
  search: string | undefined,
): readonly VersionedDefinition[] {
  const needle = search?.trim().toLowerCase()
  if (!needle) return definitions
  return definitions.filter((d) => d.name.toLowerCase().includes(needle))
}

/** How one detail of a definition differs from the version before. */
export type DefinitionChange = 'added' | 'changed' | 'removed'

/** One detail of a definition version, nested details with dotted keys. */
export type DefinitionEntry = {
  key: string
  /** The value in this version; of the version before for a removed detail. */
  value: unknown
  change?: DefinitionChange
  /** The value in the version before, for a changed detail. */
  previous?: unknown
}

/**
 * The details of a definition, compared with the version before it if
 * there is one: its own details in the uploaded order, then those the
 * version before had and this one does not.
 */
export function definitionEntries(
  definition: object,
  previous?: object,
): DefinitionEntry[] {
  const current = flattenJson(jsonObject(definition))
  if (!previous) return current.map(([key, value]) => ({ key, value }))

  const before = new Map(flattenJson(jsonObject(previous)))
  const keys = new Set(current.map(([key]) => key))
  const entries = current.map(([key, value]): DefinitionEntry => {
    if (!before.has(key)) return { key, value, change: 'added' }
    const old = before.get(key)
    // JSON comparison: a detail may be an array or change its type.
    return JSON.stringify(old) === JSON.stringify(value)
      ? { key, value }
      : { key, value, change: 'changed', previous: old }
  })
  const removed = [...before]
    .filter(([key]) => !keys.has(key))
    .map(([key, value]): DefinitionEntry => ({ key, value, change: 'removed' }))
  return [...entries, ...removed]
}

/** A version of a definition with what it changed. */
export type VersionRow = {
  version: DefinitionVersion
  /** The version before it; undefined for the first one. */
  previous?: DefinitionVersion
  /** The keys of the details it changed, by kind of change. */
  changes: Record<DefinitionChange, string[]>
}

/** The versions of a definition, newest first, each with its changes. */
export function versionRows(definition: VersionedDefinition): VersionRow[] {
  const ascending = [...definition.versions].sort(
    (a, b) => a.version - b.version,
  )
  return ascending
    .map((version, index): VersionRow => {
      const previous = index > 0 ? ascending[index - 1] : undefined
      const changes: VersionRow['changes'] = {
        changed: [],
        added: [],
        removed: [],
      }
      for (const entry of definitionEntries(
        version.definition,
        previous?.definition,
      )) {
        if (entry.change) changes[entry.change].push(entry.key)
      }
      return { version, previous, changes }
    })
    .reverse()
}

/**
 * The row of the version in the URL, or of the latest version if the URL
 * names none. Undefined if the definition has no such version.
 */
export function selectedRow(
  rows: readonly VersionRow[],
  version: number | undefined,
): VersionRow | undefined {
  return version === undefined
    ? rows[0]
    : rows.find((row) => row.version.version === version)
}

/** How many test cases each version of a scenario has. */
export function casesPerVersion(
  testCases: readonly TestCaseSummary[],
): Map<number, number> {
  const counts = new Map<number, number>()
  for (const tc of testCases) {
    counts.set(tc.scenario_version, (counts.get(tc.scenario_version) ?? 0) + 1)
  }
  return counts
}

/** The test cases of one scenario version, optionally only those with a tag. */
export function casesOfVersion(
  testCases: readonly TestCaseSummary[],
  version: number,
  tag?: string,
): TestCaseSummary[] {
  return testCases.filter(
    (tc) =>
      tc.scenario_version === version &&
      (tag === undefined || tc.tags.includes(tag)),
  )
}
