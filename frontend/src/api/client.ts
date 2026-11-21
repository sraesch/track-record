// Typed client for the UI API (/ui/api). Types are generated from
// backend/openapi/ui.json into schema.d.ts (npm run generate:api).
import createClient from 'openapi-fetch'
import i18n from '../i18n'
import type { components, paths } from './schema'

export type { paths }
export type Schemas = components['schemas']
export type Problem = Schemas['Problem']
export type RunSummary = Schemas['RunSummary']
export type RunStatus = Schemas['RunStatus']
export type RunTrigger = Schemas['RunTrigger']
export type Validity = Schemas['Validity']
export type RunListQuery = NonNullable<
  paths['/ui/api/v1/runs']['get']['parameters']['query']
>

export const api = createClient<paths>({
  // Same origin as the app (nginx or the Vite dev server proxies /ui/api).
  baseUrl: window.location.origin,
  // Looked up on every call, so tests can stub the global fetch.
  fetch: (request) => globalThis.fetch(request),
})

/** A failed API call; `code` is the backend's stable error code. */
export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, problem: Partial<Problem> | undefined) {
    super(
      problem?.detail ??
        problem?.title ??
        i18n.t('common.httpError', { status }),
    )
    this.name = 'ApiError'
    this.status = status
    this.code = problem?.code ?? 'unknown'
  }
}

/** Returns the data of an openapi-fetch result or throws an ApiError. */
export function unwrap<T>(result: {
  data?: T
  error?: unknown
  response: Response
}): T {
  if (result.error !== undefined || result.data === undefined) {
    throw new ApiError(
      result.response.status,
      result.error as Partial<Problem> | undefined,
    )
  }
  return result.data
}
