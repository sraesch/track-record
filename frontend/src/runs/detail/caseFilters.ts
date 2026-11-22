import type { CaseExecutionSummary } from '../../api/client'
import { formatJsonValue } from '../../format'
import { jsonObject } from './json'

// Filters of the case executions on a run's page. They live in the URL so a
// link reproduces the view (FR-21):
//
//   /runs/:runId?scenario=orbit&device=pixel-8-lab-1&search=meshopt&superseded=true
//
// Scenario and device match exactly (the device after aliases); `search`
// matches any part of a case: scenario, parameters, assets and devices.
// Superseded case executions (replaced by a correction, FR-6) are hidden
// unless `superseded=true`.

export type CaseFilters = {
  scenario?: string
  device?: string
  search?: string
  superseded: boolean
}

const text = (value: string | null) => value?.trim() || undefined

/** Reads the filters from a URL query; unknown values are ignored. */
export function parseCaseFilters(params: URLSearchParams): CaseFilters {
  return {
    scenario: text(params.get('scenario')),
    device: text(params.get('device')),
    // Not trimmed: the field shows what is typed, spaces included.
    search: params.get('search') || undefined,
    superseded: params.get('superseded') === 'true',
  }
}

/** Writes the filters as a URL query; defaults are left out. */
export function caseFiltersToParams(filters: CaseFilters): URLSearchParams {
  const params = new URLSearchParams()
  if (filters.scenario) params.set('scenario', filters.scenario)
  if (filters.device) params.set('device', filters.device)
  if (filters.search) params.set('search', filters.search)
  if (filters.superseded) params.set('superseded', 'true')
  return params
}

/** True if a filter narrows the list (showing superseded ones does not). */
export function hasActiveCaseFilters(filters: CaseFilters): boolean {
  return Boolean(filters.scenario || filters.device || filters.search?.trim())
}

/** Everything `search` looks at in a case execution, in lower case. */
function searchText(ce: CaseExecutionSummary): string {
  const { test_case: tc, client_device: device } = ce
  const parameters = (values: object) =>
    Object.entries(jsonObject(values)).map(
      ([name, value]) => `${name}=${formatJsonValue(value)}`,
    )
  return [
    tc.scenario,
    ...parameters(tc.resolved_parameters),
    ...parameters(tc.parameters),
    ...tc.assets.flatMap((a) => [a.name, a.version ?? '']),
    ...tc.tags,
    device.resolved_name,
    device.name,
  ]
    .join('\n')
    .toLowerCase()
}

/** The case executions that match the filters, in their order. */
export function filterCaseExecutions(
  caseExecutions: readonly CaseExecutionSummary[],
  filters: CaseFilters,
): CaseExecutionSummary[] {
  const terms = (filters.search ?? '').toLowerCase().split(/\s+/).filter(Boolean)
  return caseExecutions.filter((ce) => {
    if (filters.scenario && ce.test_case.scenario !== filters.scenario)
      return false
    if (filters.device && ce.client_device.resolved_name !== filters.device)
      return false
    if (terms.length > 0) {
      const haystack = searchText(ce)
      if (!terms.every((term) => haystack.includes(term))) return false
    }
    return true
  })
}
