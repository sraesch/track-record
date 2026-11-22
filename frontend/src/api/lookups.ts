// Reference data shared by the views' filters: names of suites, client
// devices, scenarios, assets and metrics, and the parts of SUT versions.
// They change rarely, so they are refetched less often than the views' own
// data.
import { useQuery } from '@tanstack/react-query'
import { api, unwrap } from './client'

const lookupStaleTime = 5 * 60_000

const uniqueSorted = (values: string[]) => [...new Set(values)].sort()

// Selectors at module scope, so TanStack Query reruns them only when the
// data changes, not on every render.
const deviceNames = (devices: { resolved_name: string }[]) =>
  uniqueSorted(devices.map((d) => d.resolved_name))
const names = (items: { name: string }[]) =>
  uniqueSorted(items.map((i) => i.name))

type SutVersion = {
  commit: string
  branch?: string | null
  tag?: string | null
  build_config?: string | null
}
/** The values of one optional part of the SUT versions, each once. */
const sutPart =
  (part: 'branch' | 'tag' | 'build_config') => (versions: SutVersion[]) =>
    uniqueSorted(versions.flatMap((v) => (v[part] ? [v[part]] : [])))
const branchNames = sutPart('branch')
const sutTags = sutPart('tag')
const buildConfigs = sutPart('build_config')
const commits = (versions: SutVersion[]) =>
  uniqueSorted(versions.map((v) => v.commit))

export function useSuiteNames() {
  return useQuery({
    queryKey: ['suites'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/suites', { signal })),
    select: names,
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

/** All known SUT versions, reduced by `select`; one request for all parts. */
function useSutVersions(select: (versions: SutVersion[]) => string[]) {
  return useQuery({
    queryKey: ['sut-versions'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/sut-versions', { signal })),
    select,
    staleTime: lookupStaleTime,
  })
}

/** Branches of all known SUT versions, each once. */
export const useBranches = () => useSutVersions(branchNames)
/** Full commit hashes of all known SUT versions. */
export const useCommits = () => useSutVersions(commits)
/** Tags of all known SUT versions (e.g. releases), each once. */
export const useSutTags = () => useSutVersions(sutTags)
/** Build configurations of all known SUT versions, each once. */
export const useBuildConfigs = () => useSutVersions(buildConfigs)

/** Scenario names (all versions of a scenario share one name). */
export function useScenarioNames() {
  return useQuery({
    queryKey: ['scenarios'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/scenarios', { signal })),
    select: names,
    staleTime: lookupStaleTime,
  })
}

/** Asset names, each once (an asset can have several versions). */
export function useAssetNames() {
  return useQuery({
    queryKey: ['assets'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/assets', { signal })),
    select: names,
    staleTime: lookupStaleTime,
  })
}

/** Names of the uploaded metrics (not derived metrics). */
export function useMetricNames() {
  return useQuery({
    queryKey: ['metrics'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/metrics', { signal })),
    select: names,
    staleTime: lookupStaleTime,
  })
}
