import { keepPreviousData, useInfiniteQuery } from '@tanstack/react-query'
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
    // Keep the previous rows on screen while another filter loads.
    placeholderData: keepPreviousData,
  })
}
