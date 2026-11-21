import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import { api, type RunListQuery, unwrap } from '../api/client'

export const runPageSize = 50

/** Runs matching a query, newest measured start first, page by page. */
export function useRunList(query: RunListQuery) {
  return useInfiniteQuery({
    queryKey: ['runs', 'list', query],
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
  })
}

export function useSuites() {
  return useQuery({
    queryKey: ['suites'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/suites', { signal })),
    select: (suites) => suites.map((s) => s.name).sort(),
  })
}

/** Client device names after aliases (FR-14), each once. */
export function useClientDeviceNames() {
  return useQuery({
    queryKey: ['client-devices'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/client-devices', { signal })),
    select: (devices) =>
      [...new Set(devices.map((d) => d.resolved_name))].sort(),
  })
}

/** Branches of all known SUT versions, each once. */
export function useBranches() {
  return useQuery({
    queryKey: ['sut-versions'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/sut-versions', { signal })),
    select: (versions) =>
      [
        ...new Set(
          versions.flatMap((v) => (v.branch ? [v.branch] : [])),
        ),
      ].sort(),
  })
}
