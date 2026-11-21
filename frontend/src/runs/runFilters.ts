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
//
// `from` and `to` are calendar days in the browser's time zone, both
// inclusive. They filter on the measured start time of a run.

export const runStatuses: readonly RunStatus[] = [
  'running',
  'completed',
  'failed',
  'aborted',
]
export const runTriggers: readonly RunTrigger[] = ['nightly', 'manual', 'ci']
export const validities: readonly Validity[] = ['valid', 'invalid', 'all']

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
}

const isDay = (value: string | null): value is string =>
  value !== null && /^\d{4}-\d{2}-\d{2}$/.test(value)

function oneOf<T extends string>(
  allowed: readonly T[],
  value: string | null,
): T | undefined {
  return allowed.find((a) => a === value)
}

const text = (value: string | null) => value?.trim() || undefined

/** Reads the filters from a URL query; unknown or malformed values are ignored. */
export function parseRunFilters(params: URLSearchParams): RunFilters {
  return {
    suite: text(params.get('suite')),
    status: runStatuses.filter((s) => params.getAll('status').includes(s)),
    trigger: oneOf(runTriggers, params.get('trigger')),
    branch: text(params.get('branch')),
    device: text(params.get('device')),
    tags: [...new Set(params.getAll('tag').map((t) => t.trim()))].filter(
      Boolean,
    ),
    validity: oneOf(validities, params.get('validity')) ?? 'valid',
    from: isDay(params.get('from')) ? params.get('from')! : undefined,
    to: isDay(params.get('to')) ? params.get('to')! : undefined,
  }
}

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
  return params
}

export function hasActiveFilters(filters: RunFilters): boolean {
  return runFiltersToParams(filters).size > 0
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
  }
}
