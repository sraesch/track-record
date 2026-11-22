import { type SuitesView, suitesViewToParams } from './suiteModel'

/** URL of a view of the suites & scenarios page (FR-21). */
export function suitesHref(view: SuitesView): string {
  const query = suitesViewToParams(view).toString()
  return query ? `/suites?${query}` : '/suites'
}

/** URL of a suite version, e.g. the one a run was executed with. */
export const suiteVersionHref = (name: string, version: number) =>
  suitesHref({ tab: 'suites', name, version })

/** URL of a test case: its scenario version with the test case selected. */
export const testCaseHref = (testCase: {
  id: number
  scenario: string
  scenario_version: number
}) =>
  suitesHref({
    tab: 'scenarios',
    name: testCase.scenario,
    version: testCase.scenario_version,
    testCase: testCase.id,
  })
