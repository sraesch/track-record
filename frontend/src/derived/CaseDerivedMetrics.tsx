import Box from '@mui/material/Box'
import Link from '@mui/material/Link'
import Skeleton from '@mui/material/Skeleton'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { Fragment, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import {
  ApiError,
  type CaseExecutionDetail,
  type EvaluateRequest,
  type EvaluateResponse,
} from '../api/client'
import { ErrorAlert } from '../pages/ErrorAlert'
import { Section } from '../pages/Section'
import { liveRunInterval } from '../runs/queries'
import { allMissing, type DerivedRow, resultsBySample } from './derivedModel'
import {
  ScalarTable,
  SeriesResultChart,
  type ValueColumn,
} from './EvaluationViews'
import { derivedHref, derivedMetricHref } from './paths'
import { useDerivedMetrics, useEvaluations } from './queries'
import { previewPoints } from './EvaluationPreview'

type Evaluated = { row: DerivedRow; response: EvaluateResponse }
type Skipped = { row: DerivedRow; reason: string }

/**
 * The latest version of every derived metric (FR-17), evaluated on the
 * samples of a case execution: scalars in a table, one column per metric,
 * series as charts. Metrics whose inputs this case does not have are only
 * named, as are those that could not be evaluated.
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
      rows.map(
        (row): EvaluateRequest => ({
          case_execution_id: caseExecution.id,
          name: row.name,
          max_points: previewPoints,
        }),
      ),
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
      <Link
        component={RouterLink}
        to={derivedMetricHref(row.name)}
        sx={{ fontFamily: 'monospace' }}
      >
        {row.name}
      </Link>
    ),
    unit: response.unit,
    results: resultsBySample(response),
  }))

  return (
    <Section
      title={t('caseExecution.derived.title')}
      flush
      actions={
        <Link component={RouterLink} to="/derived-metrics" variant="body2">
          {t('caseExecution.derived.manage')}
        </Link>
      }
    >
      {metrics.isError && !metrics.data ? (
        <Box sx={{ px: 2, pb: 2 }}>
          <ErrorAlert
            message={t('derived.error', { message: metrics.error.message })}
            onRetry={() => metrics.refetch()}
          />
        </Box>
      ) : metrics.isPending ? (
        <Box sx={{ px: 2, pb: 2 }}>
          <Skeleton variant="rounded" height={80} aria-label={t('common.loading')} />
        </Box>
      ) : rows.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2, pb: 2 }}>
          {t('caseExecution.derived.none')}{' '}
          <Link component={RouterLink} to={derivedHref({ edit: 'new' })}>
            {t('caseExecution.derived.define')}
          </Link>
        </Typography>
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
            <Box sx={{ px: 2, pt: 1 }}>
              <Skeleton
                variant="rounded"
                height={columns.length > 0 ? 24 : 80}
                aria-label={t('caseExecution.derived.evaluating', {
                  count: pending,
                })}
              />
            </Box>
          )}
          {series.length > 0 && (
            <Box sx={{ display: 'grid', gap: 3, px: 2, pt: 2 }}>
              {series.map(({ row, response }) => (
                <SeriesResultChart
                  key={row.name}
                  title={row.name}
                  response={response}
                />
              ))}
            </Box>
          )}
          {(nothingShown ||
            notApplicable.length > 0 ||
            failed.length > 0) && (
            <Box sx={{ display: 'grid', gap: 0.5, px: 2, py: 1.5 }}>
              {nothingShown && (
                <Typography variant="body2" color="text.secondary">
                  {t('caseExecution.derived.noneApply')}
                </Typography>
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
            </Box>
          )}
          {/* Space below the charts, which have no table border under them. */}
          {series.length > 0 &&
            notApplicable.length === 0 &&
            failed.length === 0 && <Box sx={{ height: 16 }} />}
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
    <Typography variant="body2" color={error ? 'error' : 'text.secondary'}>
      {label}{' '}
      {skipped.map(({ row, reason }, i) => (
        <Fragment key={row.name}>
          {i > 0 && ', '}
          <Tooltip title={reason} describeChild>
            <Link
              component={RouterLink}
              to={derivedMetricHref(row.name)}
              color="inherit"
              sx={{ fontFamily: 'monospace' }}
            >
              {row.name}
            </Link>
          </Tooltip>
        </Fragment>
      ))}
    </Typography>
  )
}
