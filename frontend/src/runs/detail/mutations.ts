// Changes to a run from its page: tags, annotations, validity (FR-9) and
// hard delete (FR-10). After a change, everything about runs is refetched
// (the run, its case executions and the run list), so every view shows it.
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, expectOk, unwrap } from '../../api/client'
import { runKeys } from '../queries'

function useRunMutation<T>(fn: (variables: T) => Promise<unknown>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSettled: () => queryClient.invalidateQueries({ queryKey: runKeys.all }),
  })
}

export function useAddRunTag(runId: string) {
  return useRunMutation(async (tag: string) =>
    expectOk(
      await api.PUT('/ui/api/v1/runs/{run_id}/tags/{tag}', {
        params: { path: { run_id: runId, tag } },
      }),
    ),
  )
}

export function useRemoveRunTag(runId: string) {
  return useRunMutation(async (tag: string) =>
    expectOk(
      await api.DELETE('/ui/api/v1/runs/{run_id}/tags/{tag}', {
        params: { path: { run_id: runId, tag } },
      }),
    ),
  )
}

export function useCreateRunAnnotation(runId: string) {
  return useRunMutation(async (body: string) =>
    unwrap(
      await api.POST('/ui/api/v1/annotations', {
        body: { body, run_id: runId },
      }),
    ),
  )
}

export function useUpdateAnnotation() {
  return useRunMutation(async ({ id, body }: { id: string; body: string }) =>
    unwrap(
      await api.PATCH('/ui/api/v1/annotations/{annotation_id}', {
        params: { path: { annotation_id: id } },
        body: { body },
      }),
    ),
  )
}

export function useDeleteAnnotation() {
  return useRunMutation(async (id: string) =>
    expectOk(
      await api.DELETE('/ui/api/v1/annotations/{annotation_id}', {
        params: { path: { annotation_id: id } },
      }),
    ),
  )
}

/** Marks the run invalid with a reason (FR-9). */
export function useInvalidateRun(runId: string) {
  return useRunMutation(async (reason: string) =>
    expectOk(
      await api.POST('/ui/api/v1/runs/{run_id}/invalidate', {
        params: { path: { run_id: runId } },
        body: { reason },
      }),
    ),
  )
}

/** Marks the run valid again (FR-9). */
export function useValidateRun(runId: string) {
  return useRunMutation(async () =>
    expectOk(
      await api.POST('/ui/api/v1/runs/{run_id}/validate', {
        params: { path: { run_id: runId } },
      }),
    ),
  )
}

/**
 * Hard-deletes the run (FR-10); `confirmRunId` must repeat its ID. The
 * caller leaves the run's page in `onSuccess`; the run is then dropped from
 * the cache instead of refetched, and the run list is refetched.
 */
export function useDeleteRun(runId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (confirmRunId: string) =>
      expectOk(
        await api.POST('/ui/api/v1/runs/{run_id}/delete', {
          params: { path: { run_id: runId } },
          body: { confirm_run_id: confirmRunId },
        }),
      ),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: runKeys.run(runId) })
      return queryClient.invalidateQueries({ queryKey: runKeys.lists })
    },
  })
}
