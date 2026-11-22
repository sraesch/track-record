import type { Alias } from '../api/client'

// Aliases (FR-14) map a value of a dimension to another value. Dimensions
// are `client_device` (values are names, JSON strings) and
// `parameter:<name>` for a case parameter, whose values keep their JSON
// type: 10 and "10" are different values.

export const deviceDimension = 'client_device'
const parameterPrefix = 'parameter:'

export type Dimension = { kind: 'device' } | { kind: 'parameter'; name: string }

export function parseDimension(dimension: string): Dimension | undefined {
  if (dimension === deviceDimension) return { kind: 'device' }
  if (dimension.startsWith(parameterPrefix)) {
    const name = dimension.slice(parameterPrefix.length).trim()
    if (name) return { kind: 'parameter', name }
  }
  return undefined
}

export const parameterDimension = (name: string) =>
  `${parameterPrefix}${name.trim()}`

/** An alias value: a JSON scalar. */
export type AliasValue = string | number | boolean

export type ValueType = 'string' | 'number' | 'boolean'
export const valueTypes: readonly ValueType[] = ['string', 'number', 'boolean']

export function isAliasValue(value: unknown): value is AliasValue {
  return (
    typeof value === 'string' ||
    (typeof value === 'number' && Number.isFinite(value)) ||
    typeof value === 'boolean'
  )
}

export function typeOf(value: AliasValue): ValueType {
  return typeof value as ValueType
}

/** Same type and value, as the backend compares them (10 equals 10.0). */
export function sameValue(a: AliasValue, b: AliasValue): boolean {
  return typeof a === typeof b && a === b
}

/** A value as it is typed into a field (not localized, so it parses back). */
export function valueText(value: AliasValue): string {
  return String(value)
}

/**
 * The text of a field as a value of the given type; undefined if it is
 * empty or not a value of that type. Text is trimmed: an uploaded name
 * never starts or ends with spaces that matter.
 */
export function parseValue(
  text: string,
  type: ValueType,
): AliasValue | undefined {
  const trimmed = text.trim()
  if (!trimmed) return undefined
  if (type === 'string') return trimmed
  if (type === 'boolean') {
    if (trimmed === 'true') return true
    if (trimmed === 'false') return false
    return undefined
  }
  // Number(" ") and Number("0x1f") are numbers; only plain decimals count.
  if (!/^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/.test(trimmed)) return undefined
  const n = Number(trimmed)
  return Number.isFinite(n) ? n : undefined
}

/** A key for a value that keeps its type apart: "s:10" vs "n:10". */
export const valueKey = (value: AliasValue) => `${typeOf(value)[0]}:${value}`

/** An alias as the page shows it; `target` is undefined once removed. */
export type AliasRow = {
  id: number
  dimension: string
  source: AliasValue
  target?: AliasValue
  version: number
  updatedAt: string
}

/** The aliases of the API with their values checked; others are dropped. */
export function toRows(aliases: readonly Alias[]): AliasRow[] {
  return aliases.flatMap((a) => {
    const source: unknown = a.source
    const target: unknown = a.target
    if (!isAliasValue(source)) return []
    return [
      {
        id: a.id,
        dimension: a.dimension,
        source,
        target: isAliasValue(target) ? target : undefined,
        version: a.version,
        updatedAt: a.updated_at,
      },
    ]
  })
}

/** Longest chain followed, as in the backend (service/query.rs). */
const maxChain = 32

/**
 * The value a source ends up as when aliases form a chain (a → b → c), as
 * the backend resolves it. Equal to the target when there is no chain.
 */
export function resolve(
  rows: readonly AliasRow[],
  dimension: string,
  value: AliasValue,
): AliasValue {
  const targets = new Map<string, AliasValue>()
  for (const row of rows) {
    if (row.dimension === dimension && row.target !== undefined) {
      targets.set(valueKey(row.source), row.target)
    }
  }
  let current = value
  const seen = new Set([valueKey(current)])
  for (let i = 0; i < maxChain; i++) {
    const next = targets.get(valueKey(current))
    if (next === undefined || seen.has(valueKey(next))) break
    seen.add(valueKey(next))
    current = next
  }
  return current
}

// The view's state in the URL (FR-21):
//
//   /aliases?dimension=parameter%3Asize&q=pixel&removed=1

export type AliasFilters = {
  /** A dimension, or undefined for all of them. */
  dimension?: string
  /** Free text, matched against values and parameter names. */
  search?: string
  /** Also list removed aliases, so they can be set again. */
  removed: boolean
}

export function parseAliasFilters(params: URLSearchParams): AliasFilters {
  const dimension = params.get('dimension') ?? ''
  return {
    dimension: parseDimension(dimension) ? dimension : undefined,
    search: params.get('q')?.trim() || undefined,
    removed: params.get('removed') === '1',
  }
}

export function aliasFiltersToParams(filters: AliasFilters): URLSearchParams {
  const params = new URLSearchParams()
  if (filters.dimension) params.set('dimension', filters.dimension)
  if (filters.search?.trim()) params.set('q', filters.search.trim())
  if (filters.removed) params.set('removed', '1')
  return params
}

export function filterRows(
  rows: readonly AliasRow[],
  filters: AliasFilters,
): AliasRow[] {
  const search = filters.search?.toLowerCase()
  return rows.filter(
    (row) =>
      (filters.removed || row.target !== undefined) &&
      (!filters.dimension || row.dimension === filters.dimension) &&
      (!search ||
        [row.dimension, row.source, row.target ?? ''].some((v) =>
          String(v).toLowerCase().includes(search),
        )),
  )
}
