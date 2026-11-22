import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
} from '@tanstack/react-query'
import { api, type RunListQuery, unwrap } from '../api/client'

export const runPageSize = 50

/** How often a running run is refetched while its page is open. */
export const liveRunInterval = 15_000

// Query keys. Everything about one run starts with `runKeys.run(id)`, so a
// change to the run refreshes, and its deletion removes, all of it at once.
export const runKeys = {
  all: ['runs'] as const,
  lists: ['runs', 'list'] as const,
  list: (query: RunListQuery) => ['runs', 'list', query] as const,
  run: (runId: string) => ['runs', 'detail', runId] as const,
  caseExecutions: (runId: string, includeSuperseded: boolean) =>
    ['runs', 'detail', runId, 'case-executions', includeSuperseded] as const,
  caseExecution: (runId: string, caseExecutionId: string) =>
    ['runs', 'detail', runId, 'case-execution', caseExecutionId] as const,
}

/** Runs matching a query, newest measured start first, page by page. */
export function useRunList(query: RunListQuery) {
  return useInfiniteQuery({
    queryKey: runKeys.list(query),
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/runs', {
          params: {
            query: { ...query, limit: runPageSize, cursor: pageParam },
          },
          signal,
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    // Keep the previous rows on screen while another filter loads.
    placeholderData: keepPreviousData,
  })
}

/**
 * One run with its tags, annotations and attachments. A running run is
 * refetched regularly, so its page follows the upload (FR-1).
 */
export function useRun(runId: string) {
  return useQuery({
    queryKey: runKeys.run(runId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/runs/{run_id}', {
          params: { path: { run_id: runId } },
          signal,
        }),
      ),
    refetchInterval: (query) =>
      query.state.data?.status === 'running' ? liveRunInterval : false,
  })
}

/**
 * The case executions of a run. Superseded ones (replaced by a correction,
 * FR-6) only on request. `live` refetches them like a running run.
 */
export function useCaseExecutions(
  runId: string,
  includeSuperseded: boolean,
  live: boolean,
) {
  return useQuery({
    queryKey: runKeys.caseExecutions(runId, includeSuperseded),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/runs/{run_id}/case-executions', {
          params: {
            path: { run_id: runId },
            query: includeSuperseded
              ? { include_superseded: true }
              : undefined,
          },
          signal,
        }),
      ),
    refetchInterval: live ? liveRunInterval : false,
    // Keep the rows while the other set (with or without superseded) loads,
    // but never show the rows of another run.
    placeholderData: (previous, previousQuery) =>
      previousQuery?.queryKey[2] === runId ? previous : undefined,
  })
}

/**
 * One case execution with its samples, measurements and series metadata.
 * `live` refetches it while its run is running, as samples keep coming.
 */
export function useCaseExecution(
  runId: string,
  caseExecutionId: string,
  live: boolean,
) {
  return useQuery({
    queryKey: runKeys.caseExecution(runId, caseExecutionId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/case-executions/{case_execution_id}', {
          params: { path: { case_execution_id: caseExecutionId } },
          signal,
        }),
      ),
    refetchInterval: live ? liveRunInterval : false,
  })
}
