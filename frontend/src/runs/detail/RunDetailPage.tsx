import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import Alert from '@mui/material/Alert'
import AlertTitle from '@mui/material/AlertTitle'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useParams, useSearchParams } from 'react-router'
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
import { useCaseExecutions, useRun } from '../queries'
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
    <Button
      component={Link}
      to="/runs"
      size="small"
      startIcon={<ArrowBackIcon />}
      sx={{ mb: 1, ml: -1 }}
    >
      {t('runDetail.back')}
    </Button>
  )

  if (!run.data) {
    return (
      <>
        <PageHeader title={t('runDetail.titleLoading')} before={back} />
        {run.isPending ? (
          <LinearProgress aria-label={t('common.loading')} />
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
          sx={{ mb: 2 }}
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
        <RunOverview run={run} />
        <Box sx={{ display: 'grid', gap: 2, minWidth: 0 }}>
          <RunTags run={run} />
          <RunAnnotations run={run} />
          <JsonDetails
            title={t('runDetail.environment.title')}
            emptyText={t('runDetail.environment.empty')}
            details={run.shared_environment}
          />
        </Box>
        {/* Above the case executions, which can be a long list. */}
        <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
          <RunAttachments
            attachments={run.attachments}
            attachedTo={(attachment) => (
              <AttachedTo attachment={attachment} caseExecutions={caseById} />
            )}
            previewHref={preview.previewHref}
          />
        </Box>
        <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
          <CaseExecutions
            runId={run.id}
            query={caseExecutions}
            filters={filters}
            onChange={setFilters}
          />
        </Box>
      </Box>
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
      severity="warning"
      sx={{ mb: 2 }}
      action={
        <Button
          color="inherit"
          size="small"
          loading={validate.isPending}
          onClick={() => validate.mutate()}
        >
          {t('runDetail.validate.action')}
        </Button>
      }
    >
      <AlertTitle>
        {run.invalid_reason
          ? t('runDetail.validate.reason', { reason: run.invalid_reason })
          : t('runDetail.validate.noReason')}
      </AlertTitle>
      {t('runDetail.validate.text')}
      {validate.error && (
        <Box sx={{ mt: 1, color: 'error.main' }}>
          {t('runDetail.validate.error', { message: validate.error.message })}
        </Box>
      )}
    </Alert>
  )
}
