// Changes to a test case: its tags. A test case is shared by its
// executions in every run, so after a change the case executions of all
// runs and the test case lists (suites & scenarios page) are refetched,
// but not the runs or their series.
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, expectOk } from '../../api/client'
import { testCaseKeys } from '../../suites/queries'
import { isCaseExecutionKey } from '../queries'

function useTestCaseMutation<T>(fn: (variables: T) => Promise<unknown>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({
          predicate: (query) => isCaseExecutionKey(query.queryKey),
        }),
        queryClient.invalidateQueries({ queryKey: testCaseKeys.all }),
      ]),
  })
}

export function useAddTestCaseTag(testCaseId: number) {
  return useTestCaseMutation(async (tag: string) =>
    expectOk(
      await api.PUT('/ui/api/v1/test-cases/{test_case_id}/tags/{tag}', {
        params: { path: { test_case_id: testCaseId, tag } },
      }),
    ),
  )
}

export function useRemoveTestCaseTag(testCaseId: number) {
  return useTestCaseMutation(async (tag: string) =>
    expectOk(
      await api.DELETE('/ui/api/v1/test-cases/{test_case_id}/tags/{tag}', {
        params: { path: { test_case_id: testCaseId, tag } },
      }),
    ),
  )
}
