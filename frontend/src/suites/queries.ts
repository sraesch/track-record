import { useQuery } from '@tanstack/react-query'
import { api, unwrap } from '../api/client'
import { sortDefinitions, type SuitesTab } from './suiteModel'

// Suites, scenarios and test cases are created by uploads and never
// changed, so they are refetched rarely.
const staleTime = 5 * 60_000

/**
 * All suites or all scenarios with their versions, sorted by name. Shares
 * the query (and its cache) with the names of the run list filters.
 */
export function useDefinitions(tab: SuitesTab) {
  return useQuery({
    queryKey: [tab],
    queryFn: async ({ signal }) =>
      unwrap(
        tab === 'suites'
          ? await api.GET('/ui/api/v1/suites', { signal })
          : await api.GET('/ui/api/v1/scenarios', { signal }),
      ),
    select: sortDefinitions,
    staleTime,
  })
}

export const testCaseKeys = {
  all: ['test-cases'] as const,
  scenario: (name: string) => ['test-cases', { scenario: name }] as const,
}

/**
 * The test cases of all versions of a scenario, with their parameters after
 * aliases (FR-14) and their tags. Not loaded without a scenario.
 */
export function useScenarioTestCases(scenario: string | undefined) {
  return useQuery({
    queryKey: testCaseKeys.scenario(scenario ?? ''),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/test-cases', {
          params: { query: { scenario } },
          signal,
        }),
      ),
    enabled: scenario !== undefined,
    staleTime,
  })
}
