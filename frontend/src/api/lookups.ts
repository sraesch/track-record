// Reference data shared by the views' filters: suite, client device and
// branch names. They change rarely, so they are refetched less often than
// the views' own data.
import { useQuery } from '@tanstack/react-query'
import { api, unwrap } from './client'

const lookupStaleTime = 5 * 60_000

const uniqueSorted = (values: string[]) => [...new Set(values)].sort()

// Selectors at module scope, so TanStack Query reruns them only when the
// data changes, not on every render.
const suiteNames = (suites: { name: string }[]) =>
  uniqueSorted(suites.map((s) => s.name))
const deviceNames = (devices: { resolved_name: string }[]) =>
  uniqueSorted(devices.map((d) => d.resolved_name))
const branchNames = (versions: { branch?: string | null }[]) =>
  uniqueSorted(versions.flatMap((v) => (v.branch ? [v.branch] : [])))

export function useSuiteNames() {
  return useQuery({
    queryKey: ['suites'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/suites', { signal })),
    select: suiteNames,
    staleTime: lookupStaleTime,
  })
}

/** Client device names after aliases (FR-14), each once. */
export function useClientDeviceNames() {
  return useQuery({
    queryKey: ['client-devices'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/client-devices', { signal })),
    select: deviceNames,
    staleTime: lookupStaleTime,
  })
}

/** Branches of all known SUT versions, each once. */
export function useBranches() {
  return useQuery({
    queryKey: ['sut-versions'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/sut-versions', { signal })),
    select: branchNames,
    staleTime: lookupStaleTime,
  })
}
