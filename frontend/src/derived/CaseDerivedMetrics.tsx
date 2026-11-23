import { Fragment, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import {
  ApiError,
  type CaseExecutionDetail,
  type DerivedMetric,
  type EvaluateRequest,
  type EvaluateResponse,
} from '../api/client'
import { ErrorAlert } from '../pages/ErrorAlert'
import { Section } from '../pages/Section'
import { liveRunInterval } from '../runs/queries'
import { Skeleton, TextLink, Tooltip } from '../ui'
import { allMissing, resultsBySample } from './derivedModel'
import {
  ScalarTable,
  SeriesResultChart,
  type ValueColumn,
} from './EvaluationViews'
import { derivedHref, derivedMetricHref } from './paths'
import { useDerivedMetrics, useEvaluations } from './queries'
import { previewPoints } from './EvaluationPreview'
import './derived.css'

type Evaluated = { row: DerivedMetric; response: EvaluateResponse }
type Skipped = { row: DerivedMetric; reason: string }

/**
 * Every derived metric (FR-17), evaluated on the samples of a case
 * execution with its current definition: scalars in a table, one column
 * per metric, series as charts. Metrics whose inputs this case does not
 * have are only named, as are those that could not be evaluated.
 */
export function CaseDerivedMetrics({
  caseExecution,
  live,
}: {
  caseExecution: CaseExecutionDetail
  /** The run is still uploading: evaluate again regularly. */
  live: boolean
}) {
  const { t } = useTranslation()
  const metrics = useDerivedMetrics()
  const rows = useMemo(() => metrics.data ?? [], [metrics.data])
  const requests = useMemo(
    () =>
      rows.map((row): EvaluateRequest => ({
        case_execution_id: caseExecution.id,
        name: row.name,
        max_points: previewPoints,
      })),
    [rows, caseExecution.id],
  )
  const results = useEvaluations(requests, live ? liveRunInterval : false)

  const scalars: Evaluated[] = []
  const series: Evaluated[] = []
  const notApplicable: Skipped[] = []
  const failed: Skipped[] = []
  let pending = 0
  rows.forEach((row, i) => {
    const result = results[i]
    if (!result) return
    if (result.data) {
      if (allMissing(result.data)) {
        notApplicable.push({
          row,
          reason: result.data.samples[0]?.message ?? '',
        })
      } else if (result.data.kind === 'series') {
        series.push({ row, response: result.data })
      } else {
        scalars.push({ row, response: result.data })
      }
    } else if (result.error) {
      // 422: the definition does not fit the data (e.g. a series that was
      // never uploaded); anything else is a failure to show.
      const target =
        result.error instanceof ApiError && result.error.status === 422
          ? notApplicable
          : failed
      target.push({ row, reason: result.error.message })
    } else if (result.isPending) {
      pending += 1
    }
  })

  const nothingShown =
    pending === 0 && scalars.length === 0 && series.length === 0
  const columns: ValueColumn[] = scalars.map(({ row, response }) => ({
    key: row.name,
    label: (
      <TextLink to={derivedMetricHref(row.name)} className="ui-code">
        {row.name}
      </TextLink>
    ),
    unit: response.unit,
    results: resultsBySample(response),
  }))

  return (
    <Section
      title={t('caseExecution.derived.title')}
      flush
      actions={
        <TextLink to="/derived-metrics" className="small">
          {t('caseExecution.derived.manage')}
        </TextLink>
      }
    >
      {metrics.isError && !metrics.data ? (
        <div className="tr-derived-case-pad">
          <ErrorAlert
            message={t('derived.error', { message: metrics.error.message })}
            onRetry={() => metrics.refetch()}
          />
        </div>
      ) : metrics.isPending ? (
        <div className="tr-derived-case-pad">
          <Skeleton height={80} label={t('common.loading')} />
        </div>
      ) : rows.length === 0 ? (
        <p className="ui-text ui-secondary tr-derived-case-pad">
          {t('caseExecution.derived.none')}{' '}
          <TextLink to={derivedHref({ edit: 'new' })}>
            {t('caseExecution.derived.define')}
          </TextLink>
        </p>
      ) : (
        <>
          {columns.length > 0 && (
            <ScalarTable
              label={t('caseExecution.derived.title')}
              samples={caseExecution.samples}
              columns={columns}
            />
          )}
          {pending > 0 && (
            <div style={{ padding: 'var(--space-2) var(--space-4) 0' }}>
              <Skeleton
                height={columns.length > 0 ? 24 : 80}
                label={t('caseExecution.derived.evaluating', {
                  count: pending,
                })}
              />
            </div>
          )}
          {/* The last block keeps space below the charts, which have no
              table border under them. */}
          {series.length > 0 && (
            <div className="tr-derived-case-charts">
              {series.map(({ row, response }) => (
                <SeriesResultChart
                  key={row.name}
                  title={row.name}
                  response={response}
                />
              ))}
            </div>
          )}
          {(nothingShown || notApplicable.length > 0 || failed.length > 0) && (
            <div className="tr-derived-case-notes">
              {nothingShown && (
                <p className="ui-text ui-secondary">
                  {t('caseExecution.derived.noneApply')}
                </p>
              )}
              <SkippedList
                label={t('caseExecution.derived.notApplicable', {
                  count: notApplicable.length,
                })}
                skipped={notApplicable}
              />
              <SkippedList
                label={t('caseExecution.derived.failed', {
                  count: failed.length,
                })}
                skipped={failed}
                error
              />
            </div>
          )}
        </>
      )}
    </Section>
  )
}

/** Names of derived metrics left out, each with the reason on hover. */
function SkippedList({
  label,
  skipped,
  error,
}: {
  label: string
  skipped: readonly Skipped[]
  error?: boolean
}) {
  if (skipped.length === 0) return null
  return (
    <p className={error ? 'ui-text tr-derived-error' : 'ui-text ui-secondary'}>
      {label}{' '}
      {skipped.map(({ row, reason }, i) => (
        <Fragment key={row.name}>
          {i > 0 && ', '}
          <Tooltip title={reason} describes>
            <TextLink
              to={derivedMetricHref(row.name)}
              className="ui-code tr-derived-quiet-link"
            >
              {row.name}
            </TextLink>
          </Tooltip>
        </Fragment>
      ))}
    </p>
  )
}
