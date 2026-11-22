import { useQuery } from '@tanstack/react-query'
import { api, unwrap } from '../api/client'
import { toMetricRows } from './metricModel'

// Metric definitions and units change only through ingestion and
// migrations, so they are refetched rarely.
const staleTime = 5 * 60_000

/**
 * All metrics with their versions, one row per metric. Shares the query
 * (and its cache) with the metric names of the run list filters.
 */
export function useMetrics() {
  return useQuery({
    queryKey: ['metrics'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/metrics', { signal })),
    select: toMetricRows,
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
