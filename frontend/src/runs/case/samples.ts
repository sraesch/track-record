import { useTranslation } from 'react-i18next'
import type { SampleDetail } from '../../api/client'

// Shared by the measurement and series tables of a case execution.

/** A metric version as samples reference it (FR-7). */
export type MetricRef = {
  metric: string
  metric_version: number
  metric_version_id: number
}

/** Samples in repetition order. */
export const byRepetition = (samples: readonly SampleDetail[]) =>
  [...samples].sort((a, b) => a.repetition - b.repetition)

/**
 * Names metric versions: just the metric, or with its version where one
 * case execution has several versions of the same metric.
 */
export function useMetricLabel(refs: readonly MetricRef[]) {
  const { t } = useTranslation()
  const versions = new Map<string, Set<number>>()
  for (const ref of refs) {
    const seen = versions.get(ref.metric) ?? new Set()
    seen.add(ref.metric_version)
    versions.set(ref.metric, seen)
  }
  return (ref: MetricRef) =>
    (versions.get(ref.metric)?.size ?? 0) > 1
      ? t('caseExecution.metricVersion', {
          metric: ref.metric,
          version: ref.metric_version,
        })
      : ref.metric
}

/** Dims the cells of a warmup sample's row. */
export const warmupRowSx = { '& td': { color: 'text.secondary' } } as const
