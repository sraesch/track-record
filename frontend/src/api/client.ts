// Typed client for the UI API (/ui/api). Types are generated from
// backend/openapi/ui.json into schema.d.ts (npm run generate:api).
import createClient from 'openapi-fetch'
import i18n from '../i18n'
import type { components, paths } from './schema'

type Schemas = components['schemas']
export type Problem = Schemas['Problem']
export type RunSummary = Schemas['RunSummary']
export type RunDetail = Schemas['RunDetail']
export type CaseExecutionSummary = Schemas['CaseExecutionSummary']
export type CaseExecutionDetail = Schemas['CaseExecutionDetail']
export type SampleDetail = Schemas['SampleDetail']
export type TestCaseSummary = Schemas['TestCaseSummary']
export type VersionedDefinition = Schemas['VersionedDefinition']
export type DefinitionVersion = Schemas['DefinitionVersion']
export type AttachmentMeta = Schemas['AttachmentMeta']
export type Annotation = Schemas['Annotation']
export type Alias = Schemas['Alias']
export type ClientDevice = Schemas['ClientDevice']
export type Metric = Schemas['Metric']
export type SaveMetric = Schemas['SaveMetric']
export type MetricDirection = Schemas['MetricDirection']
export type MetricValueType = Schemas['MetricValueType']
export type Unit = Schemas['Unit']
export type DerivedMetric = Schemas['DerivedMetric']
export type SaveDerivedMetric = Schemas['SaveDerivedMetric']
export type ValidateExpression = Schemas['ValidateExpression']
export type Validation = Schemas['Validation']
export type EvaluateRequest = Schemas['EvaluateRequest']
export type EvaluateResponse = Schemas['EvaluateResponse']
export type SampleResult = Schemas['SampleResult']
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

/** True for an error that means the requested resource does not exist. */
export function isNotFound(error: unknown): boolean {
  return (
    error instanceof ApiError &&
    // A malformed ID in the path cannot exist either.
    (error.status === 404 || error.code === 'invalid_path')
  )
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

/** Checks an openapi-fetch result without a body (204) or throws an ApiError. */
export function expectOk(result: { error?: unknown; response: Response }) {
  if (!result.response.ok) {
    throw new ApiError(
      result.response.status,
      result.error as Partial<Problem> | undefined,
    )
  }
}
