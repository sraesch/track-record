import { ArrowLeft } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Navigate, useLocation, useParams } from 'react-router'
import {
  type AttachmentMeta,
  type CaseExecutionDetail,
  isNotFound,
  type RunDetail,
} from '../../api/client'
import { AttachmentPreview } from '../../attachments/AttachmentPreview'
import { useAttachmentPreview } from '../../attachments/useAttachmentPreview'
import { CaseDerivedMetrics } from '../../derived/CaseDerivedMetrics'
import { EmptyState } from '../../pages/EmptyState'
import { ErrorAlert } from '../../pages/ErrorAlert'
import { PageHeader } from '../../pages/PageHeader'
import { Alert, LinkButton, ProgressBar } from '../../ui'
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
import './case.css'

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
    <LinkButton
      to={runPath(runId)}
      variant="ghost"
      size="sm"
      icon={ArrowLeft}
      style={backStyle}
    >
      {t('caseExecution.back')}
    </LinkButton>
  )

  if (!caseExecution.data) {
    return (
      <>
        <PageHeader title={t('caseExecution.titleLoading')} before={back} />
        {caseExecution.isPending ? (
          <ProgressBar />
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
          style={{ marginBottom: 'var(--space-4)' }}
        />
      )}
      <CaseExecutionView caseExecution={ce} run={run.data} />
    </>
  )
}

const noAttachments: AttachmentMeta[] = []

/** Notices above the page's sections. */
const noticeStyle = { marginBottom: 'var(--space-4)' }

/** The back link sits flush with the title below it. */
const backStyle = {
  marginBottom: 'var(--space-2)',
  marginLeft: 'calc(-1 * var(--space-2))',
}

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
        <Alert
          tone="warning"
          style={noticeStyle}
          title={
            run.invalid_reason
              ? t('runDetail.validate.reason', { reason: run.invalid_reason })
              : t('runDetail.validate.noReason')
          }
        >
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
      <div className="tr-case-grid">
        <CaseOverview caseExecution={ce} run={run} />
        <div className="tr-case-column">
          <CaseParameters testCase={ce.test_case} />
          <CaseTags caseExecution={ce} />
          <CaseAnnotations testCaseId={ce.test_case.id} />
          <JsonDetails
            title={t('caseExecution.device.title')}
            emptyText={t('caseExecution.device.empty')}
            details={ce.device_details}
          />
        </div>
        <div className="tr-case-wide">
          <SampleMeasurements samples={ce.samples} />
        </div>
        <div className="tr-case-wide">
          <CaseDerivedMetrics
            caseExecution={ce}
            live={run?.status === 'running'}
          />
        </div>
        {run && (
          <div className="tr-case-wide">
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
          </div>
        )}
        <div className="tr-case-wide">
          <SampleSeries runId={ce.run_id} samples={ce.samples} />
        </div>
      </div>
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
      tone={severity}
      role={severity === 'info' ? 'status' : 'alert'}
      style={noticeStyle}
      title={title}
      action={
        <LinkButton variant="ghost" size="sm" to={to}>
          {action}
        </LinkButton>
      }
    >
      {text}
    </Alert>
  )
}
