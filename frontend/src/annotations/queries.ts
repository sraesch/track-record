// Annotations: free-text notes on a run or a test case. Run annotations come
// with the run (`useRun`); test case annotations are loaded on their own,
// as they belong to the case across all runs, not to one run.
import {
  type QueryClient,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'
import { type Annotation, api, expectOk, unwrap } from '../api/client'
import { runKeys } from '../runs/queries'

export const annotationKeys = {
  all: ['annotations'] as const,
  testCase: (testCaseId: number) =>
    ['annotations', 'test-case', testCaseId] as const,
}

/** What an annotation is on. */
export type AnnotationTarget = { run_id: string } | { test_case_id: number }

/** The annotations of a test case, oldest first. */
export function useTestCaseAnnotations(testCaseId: number) {
  return useQuery({
    queryKey: annotationKeys.testCase(testCaseId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/annotations', {
          params: { query: { test_case_id: testCaseId } },
          signal,
        }),
      ),
  })
}

/**
 * Refetches what shows annotations on `target`: everything about runs for a
 * run annotation, only the test case annotations for a test case one (so
 * the case execution page does not reload its run and series).
 */
function invalidate(
  queryClient: QueryClient,
  target: { run_id?: string | null; test_case_id?: number | null },
) {
  return queryClient.invalidateQueries({
    queryKey: target.test_case_id != null ? annotationKeys.all : runKeys.all,
  })
}

export function useCreateAnnotation(target: AnnotationTarget) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: string) =>
      unwrap(
        await api.POST('/ui/api/v1/annotations', {
          body: { body, ...target },
        }),
      ),
    onSettled: () => invalidate(queryClient, target),
  })
}

export function useUpdateAnnotation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      annotation,
      body,
    }: {
      annotation: Annotation
      body: string
    }) =>
      unwrap(
        await api.PATCH('/ui/api/v1/annotations/{annotation_id}', {
          params: { path: { annotation_id: annotation.id } },
          body: { body },
        }),
      ),
    onSettled: (_, __, { annotation }) => invalidate(queryClient, annotation),
  })
}

export function useDeleteAnnotation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (annotation: Annotation) =>
      expectOk(
        await api.DELETE('/ui/api/v1/annotations/{annotation_id}', {
          params: { path: { annotation_id: annotation.id } },
        }),
      ),
    onSettled: (_, __, annotation) => invalidate(queryClient, annotation),
  })
}
