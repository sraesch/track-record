import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import DownloadIcon from '@mui/icons-material/Download'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import Paper from '@mui/material/Paper'
import { useTranslation } from 'react-i18next'
import { Link, Navigate, useParams } from 'react-router'
import { isNotFound } from '../api/client'
import { formatBytes } from '../format'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { attachmentPath, runPath } from '../runs/paths'
import { AttachmentView } from './AttachmentView'
import { attachmentContentUrl, mediaType } from './kind'
import { useAttachment } from './queries'

/**
 * One attachment on its own page (FR-21:
 * /runs/:runId/attachments/:attachmentId), e.g. opened in a new tab from
 * the run's attachment list or preview dialog.
 */
export function AttachmentPage() {
  const { t } = useTranslation()
  const { runId = '', attachmentId = '' } = useParams()
  const attachment = useAttachment(attachmentId)

  const back = (
    <Button
      component={Link}
      to={runPath(runId)}
      size="small"
      startIcon={<ArrowBackIcon />}
      sx={{ mb: 1, ml: -1 }}
    >
      {t('attachmentPage.back')}
    </Button>
  )

  if (!attachment.data) {
    return (
      <>
        <PageHeader title={t('attachmentPage.titleLoading')} before={back} />
        {attachment.isPending ? (
          <LinearProgress aria-label={t('common.loading')} />
        ) : isNotFound(attachment.error) ? (
          <EmptyState>
            {t('attachmentPage.notFound', { attachmentId })}
          </EmptyState>
        ) : (
          <ErrorAlert
            message={t('attachmentPage.error', {
              message: attachment.error?.message,
            })}
            onRetry={() => attachment.refetch()}
          />
        )}
      </>
    )
  }

  const meta = attachment.data
  // A link with the wrong run still finds the file: correct the URL.
  if (meta.run_id !== runId) {
    return <Navigate to={attachmentPath(meta.run_id, meta.id)} replace />
  }

  return (
    <>
      <PageHeader
        title={meta.file_name}
        description={t('attachments.info', {
          type: mediaType(meta.content_type),
          size: formatBytes(meta.size_bytes),
        })}
        before={back}
        actions={
          <Button
            href={attachmentContentUrl(meta.id)}
            download={meta.file_name}
            size="small"
            variant="outlined"
            startIcon={<DownloadIcon />}
            sx={{ whiteSpace: 'nowrap' }}
          >
            {t('attachments.download')}
          </Button>
        }
      />
      <Paper variant="outlined" sx={{ p: 2, minWidth: 0 }}>
        <AttachmentView attachment={meta} fit="page" />
      </Paper>
    </>
  )
}
