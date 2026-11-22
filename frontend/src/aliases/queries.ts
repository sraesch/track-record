import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo } from 'react'
import {
  api,
  type ClientDevice,
  type TestCaseSummary,
  unwrap,
} from '../api/client'
import { jsonObject } from '../runs/detail/json'
import {
  type AliasRow,
  type AliasValue,
  deviceDimension,
  isAliasValue,
  parameterDimension,
  toRows,
  valueKey,
} from './aliasModel'

export const aliasKeys = {
  all: ['aliases'] as const,
}

/** All aliases with their current target, removed ones included. */
export function useAliases() {
  return useQuery({
    queryKey: aliasKeys.all,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/aliases', { signal })),
    select: toRows,
  })
}

export type SetAliasInput = {
  dimension: string
  source: AliasValue
  /** null removes the alias. */
  target: AliasValue | null
}

/**
 * Sets or removes an alias; each change is a new version (FR-7). Aliases
 * change what every view shows (device names, parameter values, filters,
 * trends), so afterwards every query is refetched or marked stale.
 */
export function useSetAlias() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (input: SetAliasInput) =>
      unwrap(
        await api.PUT('/ui/api/v1/aliases', {
          body: {
            dimension: input.dimension,
            // The schema calls JSON scalars `object`.
            source: input.source as unknown as Record<string, never>,
            target: input.target as unknown as Record<string, never>,
          },
        }),
      ),
    onSettled: () => queryClient.invalidateQueries(),
  })
}

/** Values found in the results, to choose from instead of typing them. */
export type KnownValues = {
  /** Parameter names, sorted. */
  parameterNames: string[]
  /** Uploaded values per dimension, without duplicates. */
  values: (dimension: string) => AliasValue[]
}

function knownValues(
  devices: readonly ClientDevice[],
  testCases: readonly TestCaseSummary[],
): KnownValues {
  const byDimension = new Map<string, Map<string, AliasValue>>()
  const add = (dimension: string, value: unknown) => {
    if (!isAliasValue(value)) return
    let values = byDimension.get(dimension)
    if (!values) byDimension.set(dimension, (values = new Map()))
    values.set(valueKey(value), value)
  }
  for (const device of devices) add(deviceDimension, device.name)
  const names = new Set<string>()
  for (const testCase of testCases) {
    for (const [name, value] of Object.entries(
      jsonObject(testCase.parameters),
    )) {
      names.add(name)
      add(parameterDimension(name), value)
    }
  }
  return {
    parameterNames: [...names].sort(),
    values: (dimension) =>
      [...(byDimension.get(dimension)?.values() ?? [])].sort((a, b) =>
        String(a).localeCompare(String(b), undefined, { numeric: true }),
      ),
  }
}

const lookupStaleTime = 5 * 60_000

/**
 * Device names and parameter values as uploaded. Loaded from the client
 * devices and the test cases; undefined until both are there.
 */
export function useKnownValues(): KnownValues | undefined {
  const devices = useQuery({
    queryKey: ['client-devices'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/client-devices', { signal })),
    staleTime: lookupStaleTime,
  })
  const testCases = useQuery({
    queryKey: ['test-cases'],
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/test-cases', { signal })),
    staleTime: lookupStaleTime,
  })
  const devicesData = devices.data
  const testCasesData = testCases.data
  return useMemo(
    () =>
      devicesData && testCasesData
        ? knownValues(devicesData, testCasesData)
        : undefined,
    [devicesData, testCasesData],
  )
}

/** True if the source of an alias occurs in the results. */
export function isKnown(known: KnownValues, row: AliasRow): boolean {
  return known
    .values(row.dimension)
    .some((value) => valueKey(value) === valueKey(row.source))
}
