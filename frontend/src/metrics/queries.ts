import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, expectOk, type SaveMetric, unwrap } from '../api/client'
import { sortMetrics } from './metricModel'

// Metrics are created by uploads and changed on the metrics page, units
// only by migrations, so both are refetched rarely.
const staleTime = 5 * 60_000

/**
 * All metrics, sorted by name. Shares the query (and its cache) with the
 * metric names of the run list filters.
 */
export function useMetrics() {
  return useQuery({
    queryKey: ['metrics'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/metrics', { signal })),
    select: sortMetrics,
    staleTime,
  })
}

/** The predefined unit catalogue (FR-13a); it only grows with releases. */
export function useUnits() {
  return useQuery({
    queryKey: ['units'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/units', { signal })),
    staleTime: Infinity,
  })
}

// A metric has one definition (ADR 0018). Its name and unit are shown with
// every result and used by derived metrics and analyses, so a change makes
// everything that was loaded out of date.
function useRefreshEverything() {
  const queryClient = useQueryClient()
  return () => queryClient.invalidateQueries()
}

/**
 * Changes the definition of a metric. The change applies to all its
 * results; stored values are not converted.
 */
export function useSaveMetric() {
  const refresh = useRefreshEverything()
  return useMutation({
    mutationFn: async ({ name, body }: { name: string; body: SaveMetric }) =>
      unwrap(
        await api.PUT('/ui/api/v1/metrics/{name}', {
          params: { path: { name } },
          body,
        }),
      ),
    onSuccess: refresh,
  })
}

/**
 * Renames a metric. The server changes the expressions of the derived
 * metrics that use it to the new name.
 */
export function useRenameMetric() {
  const refresh = useRefreshEverything()
  return useMutation({
    mutationFn: async ({ name, newName }: { name: string; newName: string }) =>
      unwrap(
        await api.POST('/ui/api/v1/metrics/{name}/rename', {
          params: { path: { name } },
          body: { name: newName },
        }),
      ),
    onSuccess: refresh,
  })
}

/** Deletes a metric without results that no derived metric uses. */
export function useDeleteMetric() {
  const refresh = useRefreshEverything()
  return useMutation({
    mutationFn: async (name: string) =>
      expectOk(
        await api.DELETE('/ui/api/v1/metrics/{name}', {
          params: { path: { name } },
        }),
      ),
    onSuccess: refresh,
  })
}
