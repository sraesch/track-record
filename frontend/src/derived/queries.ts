import {
  keepPreviousData,
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'
import {
  api,
  type EvaluateRequest,
  type EvaluateResponse,
  type SaveDerivedMetric,
  unwrap,
  type ValidateExpression,
} from '../api/client'
import { toDerivedRows } from './derivedModel'

// Everything about derived metrics starts with `derivedKeys.all`, so saving
// one refreshes the list, validations and evaluations at once: other
// expressions may use the saved metric.
export const derivedKeys = {
  all: ['derived-metrics'] as const,
  list: ['derived-metrics', 'list'] as const,
  validate: (request: ValidateExpression) =>
    ['derived-metrics', 'validate', request] as const,
  evaluate: (request: EvaluateRequest) =>
    ['derived-metrics', 'evaluate', request] as const,
}

/** All derived metrics with their versions, one row per metric (FR-17). */
export function useDerivedMetrics() {
  return useQuery({
    queryKey: derivedKeys.list,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/derived-metrics', { signal })),
    select: toDerivedRows,
  })
}

/**
 * Checks an expression (FR-17a): syntax, references, units and, with a
 * name, cycles. Undefined checks nothing. The previous result stays while
 * the next one loads, so the page does not flicker while typing.
 */
export function useValidation(request: ValidateExpression | undefined) {
  return useQuery({
    queryKey: derivedKeys.validate(request ?? { expression: '' }),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.POST('/ui/api/v1/derived-metrics/validate', {
          body: request ?? { expression: '' },
          signal,
        }),
      ),
    enabled: request !== undefined,
    placeholderData: keepPreviousData,
  })
}

const evaluate = async (request: EvaluateRequest, signal: AbortSignal) =>
  unwrap(
    await api.POST('/ui/api/v1/derived-metrics/evaluate', {
      body: request,
      signal,
    }),
  )

/** Evaluates an expression or a saved metric on the samples of a case execution. */
export function useEvaluation(request: EvaluateRequest | undefined) {
  return useQuery({
    queryKey: derivedKeys.evaluate(
      request ?? { case_execution_id: '', expression: '' },
    ),
    queryFn: ({ signal }) => evaluate(request!, signal),
    enabled: request !== undefined,
    placeholderData: keepPreviousData,
  })
}

/** One evaluation of several requests, e.g. every saved metric on one case. */
export type EvaluationResult = {
  data: EvaluateResponse | undefined
  error: Error | null
  isPending: boolean
}

/**
 * Evaluates several requests at once. `live` refetches them regularly,
 * while the run is still uploading samples.
 */
export function useEvaluations(
  requests: readonly EvaluateRequest[],
  refetchInterval: number | false,
): EvaluationResult[] {
  return useQueries({
    queries: requests.map((request) => ({
      queryKey: derivedKeys.evaluate(request),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        evaluate(request, signal),
      refetchInterval,
    })),
    combine: (results) =>
      results.map((result) => ({
        data: result.data,
        error: result.error,
        isPending: result.isPending,
      })),
  })
}

/**
 * Saves a derived metric (FR-17): a new version, or the latest one again
 * if nothing changed.
 */
export function useSaveDerivedMetric() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      name,
      body,
    }: {
      name: string
      body: SaveDerivedMetric
    }) =>
      unwrap(
        await api.PUT('/ui/api/v1/derived-metrics/{name}', {
          params: { path: { name } },
          body,
        }),
      ),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: derivedKeys.all }),
  })
}
