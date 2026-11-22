import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import Alert from '@mui/material/Alert'
import AlertTitle from '@mui/material/AlertTitle'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, Navigate, useLocation, useParams } from 'react-router'
import {
  type AttachmentMeta,
  type CaseExecutionDetail,
  isNotFound,
  type RunDetail,
} from '../../api/client'
import { AttachmentPreview } from '../../attachments/AttachmentPreview'
import { useAttachmentPreview } from '../../attachments/useAttachmentPreview'
import { EmptyState } from '../../pages/EmptyState'
import { ErrorAlert } from '../../pages/ErrorAlert'
import { PageHeader } from '../../pages/PageHeader'
import { JsonDetails } from '../detail/JsonDetails'
import { RunAttachments } from '../detail/RunAttachments'
import { caseExecutionPath, runPath } from '../paths'
import { useCaseExecution, useRun } from '../queries'
import { CaseAnnotations } from './CaseAnnotations'
import { CaseOverview } from './CaseOverview'
import { CaseParameters } from './CaseParameters'
import { CaseTags } from './CaseTags'
import { SampleMeasurements } from './SampleMeasurements'
import { SampleSeries } from './SampleSeries'

/**
 * One case execution (FR-21: /runs/:runId/cases/:caseExecutionId): what
 * was executed where, the measurements and series of every sample, the
 * client device details and the files attached to it. The open attachment
 * is in the URL query.
 */
export function CaseExecutionPage() {
  const { t } = useTranslation()
  const { runId = '', caseExecutionId = '' } = useParams()
  const location = useLocation()
  const run = useRun(runId)
  const caseExecution = useCaseExecution(
    runId,
    caseExecutionId,
    run.data?.status === 'running',
  )

  const back = (
    <Button
      component={Link}
      to={runPath(runId)}
      size="small"
      startIcon={<ArrowBackIcon />}
      sx={{ mb: 1, ml: -1 }}
    >
      {t('caseExecution.back')}
    </Button>
  )

  if (!caseExecution.data) {
    return (
      <>
        <PageHeader title={t('caseExecution.titleLoading')} before={back} />
        {caseExecution.isPending ? (
          <LinearProgress aria-label={t('common.loading')} />
        ) : isNotFound(caseExecution.error) ? (
          <EmptyState>
            {t('caseExecution.notFound', { caseExecutionId })}
          </EmptyState>
        ) : (
          <ErrorAlert
            message={t('caseExecution.error', {
              message: caseExecution.error?.message,
            })}
            onRetry={() => caseExecution.refetch()}
          />
        )}
      </>
    )
  }

  const ce = caseExecution.data
  // Reached under another run's URL: every case execution has one URL
  // (FR-21), the one under its own run.
  if (ce.run_id !== runId) {
    return (
      <Navigate
        to={`${caseExecutionPath(ce.run_id, ce.id)}${location.search}`}
        replace
      />
    )
  }

  return (
    <>
      <PageHeader
        title={t('caseExecution.title', {
          scenario: ce.test_case.scenario,
          device: ce.client_device.resolved_name,
        })}
        before={back}
      />
      {/* A failed refetch keeps the last data on screen, with the error. */}
      {caseExecution.isError && (
        <ErrorAlert
          message={
            isNotFound(caseExecution.error)
              ? t('caseExecution.gone')
              : t('caseExecution.error', {
                  message: caseExecution.error.message,
                })
          }
          onRetry={() => caseExecution.refetch()}
          sx={{ mb: 2 }}
        />
      )}
      <CaseExecutionView caseExecution={ce} run={run.data} />
    </>
  )
}

const noAttachments: AttachmentMeta[] = []

function CaseExecutionView({
  caseExecution: ce,
  run,
}: {
  caseExecution: CaseExecutionDetail
  /** Not there while it loads or if it failed to load. */
  run: RunDetail | undefined
}) {
  const { t } = useTranslation()
  // Files attached to this case execution or one of its samples.
  const attachments = useMemo(
    () =>
      run?.attachments.filter((a) => a.case_execution_id === ce.id) ??
      noAttachments,
    [run?.attachments, ce.id],
  )
  const preview = useAttachmentPreview(attachments)
  const repetitions = useMemo(
    () => new Map(ce.samples.map((s) => [s.id, s.repetition])),
    [ce.samples],
  )

  return (
    <>
      {run && !run.is_valid && (
        <Alert severity="warning" sx={{ mb: 2 }}>
          <AlertTitle>
            {run.invalid_reason
              ? t('runDetail.validate.reason', { reason: run.invalid_reason })
              : t('runDetail.validate.noReason')}
          </AlertTitle>
          {t('caseExecution.invalidRun')}
        </Alert>
      )}
      {ce.superseded_by && (
        <Notice
          severity="warning"
          title={t('caseExecution.superseded.title')}
          text={t('caseExecution.superseded.text')}
          action={t('caseExecution.superseded.action')}
          to={caseExecutionPath(ce.run_id, ce.superseded_by)}
        />
      )}
      {ce.supersedes_id && (
        <Notice
          severity="info"
          title={t('caseExecution.correction.title')}
          text={t('caseExecution.correction.text')}
          action={t('caseExecution.correction.action')}
          to={caseExecutionPath(ce.run_id, ce.supersedes_id)}
        />
      )}
      <Box
        sx={{
          display: 'grid',
          gap: 2,
          alignItems: 'start',
          gridTemplateColumns: {
            xs: 'minmax(0, 1fr)',
            lg: 'minmax(0, 2fr) minmax(0, 1fr)',
          },
        }}
      >
        <CaseOverview caseExecution={ce} run={run} />
        <Box sx={{ display: 'grid', gap: 2, minWidth: 0 }}>
          <CaseParameters testCase={ce.test_case} />
          <CaseTags caseExecution={ce} />
          <CaseAnnotations testCaseId={ce.test_case.id} />
          <JsonDetails
            title={t('caseExecution.device.title')}
            emptyText={t('caseExecution.device.empty')}
            details={ce.device_details}
          />
        </Box>
        <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
          <SampleMeasurements samples={ce.samples} />
        </Box>
        {run && (
          <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
            <RunAttachments
              attachments={attachments}
              attachedTo={(attachment) => {
                const repetition = attachment.sample_id
                  ? repetitions.get(attachment.sample_id)
                  : undefined
                return repetition === undefined
                  ? t('caseExecution.attachments.toCase')
                  : t('caseExecution.attachments.toSample', { repetition })
              }}
              previewHref={preview.previewHref}
            />
          </Box>
        )}
        <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
          <SampleSeries runId={ce.run_id} samples={ce.samples} />
        </Box>
      </Box>
      <AttachmentPreview
        attachment={preview.previewed}
        onClose={preview.closePreview}
      />
    </>
  )
}

/** A correction relation (FR-6), with a link to the other upload. */
function Notice({
  severity,
  title,
  text,
  action,
  to,
}: {
  severity: 'info' | 'warning'
  title: string
  text: string
  action: string
  to: string
}) {
  return (
    <Alert
      severity={severity}
      sx={{ mb: 2 }}
      action={
        <Button color="inherit" size="small" component={Link} to={to}>
          {action}
        </Button>
      }
    >
      <AlertTitle>{title}</AlertTitle>
      {text}
    </Alert>
  )
}
