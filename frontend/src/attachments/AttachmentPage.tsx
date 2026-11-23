import { ArrowLeft, Download } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Navigate, useParams } from 'react-router'
import { isNotFound } from '../api/client'
import { formatBytes } from '../format'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { attachmentPath, runPath } from '../runs/paths'
import { AnchorButton, LinkButton, ProgressBar, Surface } from '../ui'
import { AttachmentView } from './AttachmentView'
import { attachmentContentUrl, mediaType } from './kind'
import { useAttachment } from './queries'
import './attachments.css'

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
    <LinkButton
      to={runPath(runId)}
      variant="ghost"
      size="sm"
      icon={ArrowLeft}
      style={{
        marginBottom: 'var(--space-2)',
        marginLeft: 'calc(-1 * var(--space-2))',
      }}
    >
      {t('attachmentPage.back')}
    </LinkButton>
  )

  if (!attachment.data) {
    return (
      <>
        <PageHeader title={t('attachmentPage.titleLoading')} before={back} />
        {attachment.isPending ? (
          <ProgressBar />
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
          <AnchorButton
            icon={Download}
            href={attachmentContentUrl(meta.id)}
            download={meta.file_name}
          >
            {t('attachments.download')}
          </AnchorButton>
        }
      />
      <Surface className="tr-attachments-page">
        <AttachmentView attachment={meta} fit="page" />
      </Surface>
    </>
  )
}
