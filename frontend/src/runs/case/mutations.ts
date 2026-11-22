// Changes to the test case of a case execution: its tags. A test case is
// shared by its executions in every run, so after a change the case
// executions of all runs are refetched, but not the runs or their series.
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, expectOk } from '../../api/client'
import { isCaseExecutionKey } from '../queries'

function useTestCaseMutation<T>(fn: (variables: T) => Promise<unknown>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSettled: () =>
      queryClient.invalidateQueries({
        predicate: (query) => isCaseExecutionKey(query.queryKey),
      }),
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
