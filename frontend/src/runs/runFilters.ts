import type {
  RunListQuery,
  RunStatus,
  RunTrigger,
  Validity,
} from '../api/client'

// Filters of the run list (FR-11). They live in the URL query string so a
// link reproduces the view (FR-21):
//
//   /runs?suite=smoke&status=failed&status=aborted&branch=main
//        &device=pixel-8&tag=nightly&validity=all&from=2026-11-01&to=2026-11-20
//        &commit=168fed9…&sutTag=v1.5.0&build=release&env=3
//        &scenario=load-model&asset=city&metric=fps&param=size%3D10
//
// `from` and `to` are calendar days in the browser's time zone, both
// inclusive. They filter on the measured start time of a run. A `param` is
// `name=value`; the API reads the value as JSON (`size=10` is a number,
// `mode="10"` a string, `mode=fast` too), so parameters keep their type.

// Every value of the API's enums, in display order. A Record makes `tsc`
// fail when the backend adds a value that is missing here.
const keysOf = <T extends string>(record: Record<T, true>) =>
  Object.keys(record) as T[]

export const runStatuses = keysOf<RunStatus>({
  running: true,
  completed: true,
  failed: true,
  aborted: true,
})
export const runTriggers = keysOf<RunTrigger>({
  nightly: true,
  manual: true,
  ci: true,
})
export const validities = keysOf<Validity>({
  valid: true,
  invalid: true,
  all: true,
})

export type RunFilters = {
  suite?: string
  status: RunStatus[]
  trigger?: RunTrigger
  branch?: string
  device?: string
  tags: string[]
  /** Valid runs only unless asked otherwise (FR-9). */
  validity: Validity
  /** YYYY-MM-DD */
  from?: string
  /** YYYY-MM-DD */
  to?: string
  // The more specific filters, shown on request.
  /** Full commit hash of the SUT version. */
  commit?: string
  /** Tag of the SUT version, e.g. a release (not a run tag). */
  sutTag?: string
  buildConfig?: string
  /** ID of the shared environment. */
  environment?: number
  scenario?: string
  asset?: string
  metric?: string
  /** `name=value` each. */
  params: string[]
}

/** The filters behind "More filters". */
export const moreFilterKeys = [
  'commit',
  'sutTag',
  'buildConfig',
  'environment',
  'scenario',
  'asset',
  'metric',
  'params',
] as const satisfies readonly (keyof RunFilters)[]

/** Runs older than this are not expected; also catches half-typed years. */
const firstYear = 1970

/** A real calendar day as YYYY-MM-DD (not 2026-02-31), from `firstYear` on. */
export function isDay(value: string | null): value is string {
  const match = value?.match(/^(\d{4})-(\d{2})-(\d{2})$/)
  if (!match) return false
  const [y, m, d] = match.slice(1).map(Number) as [number, number, number]
  const date = new Date(y, m - 1, d)
  return (
    y >= firstYear &&
    date.getFullYear() === y &&
    date.getMonth() === m - 1 &&
    date.getDate() === d
  )
}

function oneOf<T extends string>(
  allowed: readonly T[],
  value: string | null,
): T | undefined {
  return allowed.find((a) => a === value)
}

const text = (value: string | null) => value?.trim() || undefined

const day = (value: string | null) => (isDay(value) ? value : undefined)

/** Tags trimmed, without empty ones and each once. */
export const cleanTags = (tags: string[]) =>
  [...new Set(tags.map((t) => t.trim()))].filter(Boolean)

/**
 * A parameter filter as `name=value` with name and value trimmed, or
 * undefined if it is not one (no `=` or no name). The value may be empty.
 */
export function cleanParam(param: string): string | undefined {
  const at = param.indexOf('=')
  if (at < 0) return undefined
  const name = param.slice(0, at).trim()
  return name ? `${name}=${param.slice(at + 1).trim()}` : undefined
}

/** Parameter filters cleaned, without malformed ones and each once. */
export const cleanParams = (params: string[]) => [
  ...new Set(params.flatMap((p) => cleanParam(p) ?? [])),
]

/** A positive whole number, e.g. a database ID. */
function id(value: string | null): number | undefined {
  if (!value || !/^\d+$/.test(value)) return undefined
  const n = Number(value)
  return Number.isSafeInteger(n) && n > 0 ? n : undefined
}

/** Reads the filters from a URL query; unknown or malformed values are ignored. */
export function parseRunFilters(params: URLSearchParams): RunFilters {
  return {
    suite: text(params.get('suite')),
    status: runStatuses.filter((s) => params.getAll('status').includes(s)),
    trigger: oneOf(runTriggers, params.get('trigger')),
    branch: text(params.get('branch')),
    device: text(params.get('device')),
    tags: cleanTags(params.getAll('tag')),
    validity: oneOf(validities, params.get('validity')) ?? 'valid',
    from: day(params.get('from')),
    to: day(params.get('to')),
    commit: text(params.get('commit')),
    sutTag: text(params.get('sutTag')),
    buildConfig: text(params.get('build')),
    environment: id(params.get('env')),
    scenario: text(params.get('scenario')),
    asset: text(params.get('asset')),
    metric: text(params.get('metric')),
    params: cleanParams(params.getAll('param')),
  }
}

/** No filter set: what an empty query string means. */
export const defaultRunFilters: RunFilters = parseRunFilters(
  new URLSearchParams(),
)

/** Writes the filters as a URL query; defaults are left out. */
export function runFiltersToParams(filters: RunFilters): URLSearchParams {
  const params = new URLSearchParams()
  if (filters.suite) params.set('suite', filters.suite)
  for (const s of filters.status) params.append('status', s)
  if (filters.trigger) params.set('trigger', filters.trigger)
  if (filters.branch) params.set('branch', filters.branch)
  if (filters.device) params.set('device', filters.device)
  for (const t of filters.tags) params.append('tag', t)
  if (filters.validity !== 'valid') params.set('validity', filters.validity)
  if (filters.from) params.set('from', filters.from)
  if (filters.to) params.set('to', filters.to)
  if (filters.commit) params.set('commit', filters.commit)
  if (filters.sutTag) params.set('sutTag', filters.sutTag)
  if (filters.buildConfig) params.set('build', filters.buildConfig)
  if (filters.environment) params.set('env', String(filters.environment))
  if (filters.scenario) params.set('scenario', filters.scenario)
  if (filters.asset) params.set('asset', filters.asset)
  if (filters.metric) params.set('metric', filters.metric)
  for (const p of filters.params) params.append('param', p)
  return params
}

export function hasActiveFilters(filters: RunFilters): boolean {
  // Not `.size`: Safari before 17 does not have it.
  return runFiltersToParams(filters).toString() !== ''
}

/** How many of the filters behind "More filters" are set. */
export function countMoreFilters(filters: RunFilters): number {
  return moreFilterKeys.filter((key) => {
    const value = filters[key]
    return Array.isArray(value) ? value.length > 0 : value !== undefined
  }).length
}

/** The run list with these filters, e.g. for a link from another page. */
export function runListHref(filters: Partial<RunFilters>): string {
  const query = runFiltersToParams({ ...defaultRunFilters, ...filters })
  const search = query.toString()
  return search ? `/runs?${search}` : '/runs'
}

/** Local midnight at the start of a YYYY-MM-DD day, plus `days`. */
function localMidnight(day: string, days = 0): string {
  const [y, m, d] = day.split('-').map(Number) as [number, number, number]
  return new Date(y, m - 1, d + days).toISOString()
}

/** The query for GET /ui/api/v1/runs. */
export function runFiltersToQuery(filters: RunFilters): RunListQuery {
  return {
    suite: filters.suite,
    status: filters.status.length > 0 ? filters.status : undefined,
    triggered_by: filters.trigger,
    sut_branch: filters.branch,
    client_device: filters.device,
    tag: filters.tags.length > 0 ? filters.tags : undefined,
    validity: filters.validity,
    started_after: filters.from ? localMidnight(filters.from) : undefined,
    // `to` is inclusive: everything before the next day starts.
    started_before: filters.to ? localMidnight(filters.to, 1) : undefined,
    sut_commit: filters.commit,
    sut_tag: filters.sutTag,
    sut_build_config: filters.buildConfig,
    shared_environment_id: filters.environment,
    scenario: filters.scenario,
    asset: filters.asset,
    metric: filters.metric,
    param: filters.params.length > 0 ? filters.params : undefined,
  }
}
