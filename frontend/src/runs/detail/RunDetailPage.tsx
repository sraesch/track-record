import { ArrowLeft } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams, useSearchParams } from 'react-router'
import { isNotFound, type RunDetail } from '../../api/client'
import { AttachmentPreview } from '../../attachments/AttachmentPreview'
import {
  attachmentParam,
  useAttachmentPreview,
} from '../../attachments/useAttachmentPreview'
import { formatDateTime } from '../../format'
import { EmptyState } from '../../pages/EmptyState'
import { ErrorAlert } from '../../pages/ErrorAlert'
import { PageHeader } from '../../pages/PageHeader'
import { Alert, Button, LinkButton, ProgressBar } from '../../ui'
import { useCaseExecutions, useRun } from '../queries'
import { runListHref } from '../runFilters'
import {
  type CaseFilters,
  caseFiltersToParams,
  parseCaseFilters,
} from './caseFilters'
import { CaseExecutions } from './CaseExecutions'
import { JsonDetails } from './JsonDetails'
import { useValidateRun } from './mutations'
import { RunActions } from './RunActions'
import { RunAnnotations } from './RunAnnotations'
import { AttachedTo, RunAttachments } from './RunAttachments'
import { RunOverview } from './RunOverview'
import { RunTags } from './RunTags'
import '../runs.css'

/**
 * One run (FR-21: /runs/:runId): what it is, its tags, annotations, case
 * executions, attachments and shared environment, and the actions on it.
 * The case execution filters and the open attachment are in the URL query.
 */
export function RunDetailPage() {
  const { t } = useTranslation()
  const { runId = '' } = useParams()
  const run = useRun(runId)

  const back = (
    <LinkButton
      to="/runs"
      variant="ghost"
      size="sm"
      icon={ArrowLeft}
      className="tr-runs-back"
    >
      {t('runDetail.back')}
    </LinkButton>
  )

  if (!run.data) {
    return (
      <>
        <PageHeader title={t('runDetail.titleLoading')} before={back} />
        {run.isPending ? (
          <ProgressBar label={t('common.loading')} />
        ) : isNotFound(run.error) ? (
          <EmptyState>{t('runDetail.notFound', { runId })}</EmptyState>
        ) : (
          <ErrorAlert
            message={t('runDetail.error', { message: run.error?.message })}
            onRetry={() => run.refetch()}
          />
        )}
      </>
    )
  }

  return (
    <>
      <PageHeader
        title={t('runDetail.title', {
          suite: run.data.suite.name,
          started: formatDateTime(run.data.started_at),
        })}
        before={back}
        actions={<RunActions run={run.data} />}
      />
      {/* A failed refetch keeps the last data on screen, with the error. */}
      {run.isError && (
        <ErrorAlert
          message={
            isNotFound(run.error)
              ? t('runDetail.gone')
              : t('runDetail.error', { message: run.error.message })
          }
          onRetry={() => run.refetch()}
          style={{ marginBottom: 'var(--space-4)' }}
        />
      )}
      <RunDetailView run={run.data} />
    </>
  )
}

function RunDetailView({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const filters = useMemo(() => parseCaseFilters(searchParams), [searchParams])
  const caseExecutions = useCaseExecutions(
    run.id,
    filters.superseded,
    run.status === 'running',
  )
  const caseById = useMemo(
    () => new Map((caseExecutions.data ?? []).map((ce) => [ce.id, ce])),
    [caseExecutions.data],
  )

  // The open attachment is in the URL too (FR-21), next to the filters.
  const preview = useAttachmentPreview(run.attachments)
  const setFilters = (next: CaseFilters, options?: { replace?: boolean }) => {
    const params = caseFiltersToParams(next)
    if (preview.attachmentId) {
      params.set(attachmentParam, preview.attachmentId)
    }
    setSearchParams(params, options)
  }

  return (
    <>
      {!run.is_valid && <InvalidNotice run={run} />}
      <div className="tr-runs-detail">
        <RunOverview run={run} />
        <div className="tr-runs-detail-side">
          <RunTags run={run} />
          <RunAnnotations run={run} />
          <JsonDetails
            title={t('runDetail.environment.title')}
            emptyText={t('runDetail.environment.empty')}
            details={run.shared_environment}
            actions={
              <LinkButton
                variant="ghost"
                size="sm"
                to={runListHref({ environment: run.shared_environment_id })}
              >
                {t('runDetail.environment.runs')}
              </LinkButton>
            }
          />
        </div>
        {/* Above the case executions, which can be a long list. */}
        <div className="tr-runs-detail-wide">
          <RunAttachments
            attachments={run.attachments}
            attachedTo={(attachment) => (
              <AttachedTo attachment={attachment} caseExecutions={caseById} />
            )}
            previewHref={preview.previewHref}
          />
        </div>
        <div className="tr-runs-detail-wide">
          <CaseExecutions
            runId={run.id}
            query={caseExecutions}
            filters={filters}
            onChange={setFilters}
          />
        </div>
      </div>
      <AttachmentPreview
        attachment={preview.previewed}
        onClose={preview.closePreview}
      />
    </>
  )
}

/** Why the run is invalid (FR-9), with the way back. */
function InvalidNotice({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const validate = useValidateRun(run.id)
  return (
    <Alert
      tone="warning"
      className="tr-runs-notice"
      title={
        run.invalid_reason
          ? t('runDetail.validate.reason', { reason: run.invalid_reason })
          : t('runDetail.validate.noReason')
      }
      action={
        <Button
          variant="ghost"
          size="sm"
          loading={validate.isPending}
          onClick={() => validate.mutate()}
        >
          {t('runDetail.validate.action')}
        </Button>
      }
    >
      {t('runDetail.validate.text')}
      {validate.error && (
        <div className="tr-runs-notice-error">
          {t('runDetail.validate.error', { message: validate.error.message })}
        </div>
      )}
    </Alert>
  )
}
