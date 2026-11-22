import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import Typography from '@mui/material/Typography'
import { lazy, Suspense, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { AttachmentMeta } from '../api/client'
import { formatBytes } from '../format'
import { ErrorAlert } from '../pages/ErrorAlert'
import { ErrorBoundary } from '../pages/ErrorBoundary'
import { JsonView } from './JsonView'
import {
  type AttachmentKind,
  attachmentContentUrl,
  attachmentKind,
  maxTextPreviewBytes,
} from './kind'
import { useAttachmentText } from './queries'
import { TextView } from './TextView'

// The Markdown renderer is large and rarely needed: it is loaded when the
// first Markdown attachment is opened (own chunk, see vite.config.ts).
const MarkdownView = lazy(() =>
  import('./MarkdownView').then((module) => ({ default: module.MarkdownView })),
)

/**
 * Where a preview is shown: in a dialog, images fit the window; on the
 * attachment's own page they are shown at their size (up to the width).
 */
export type PreviewFit = 'dialog' | 'page'

type Props = {
  attachment: AttachmentMeta
  fit: PreviewFit
}

/**
 * An attachment shown by the app (ADR 0013): images as they are, Markdown
 * formatted, JSON highlighted, other text as it is. Other files can only
 * be downloaded. A preview that fails stays here, with a way to reload.
 */
export function AttachmentView({ attachment, fit }: Props) {
  const { t } = useTranslation()
  const kind = attachmentKind(attachment)
  if (!kind) {
    return (
      <Typography color="text.secondary">
        {t('attachments.noPreview')}
      </Typography>
    )
  }
  return (
    <ErrorBoundary
      key={attachment.id}
      fallback={(error) => <PreviewFailed error={error} />}
    >
      <PreviewBody attachment={attachment} kind={kind} fit={fit} />
    </ErrorBoundary>
  )
}

/**
 * A preview that broke while rendering, most likely because its code could
 * not be loaded (the Markdown view is loaded on first use, and a new
 * release replaces it). Reloading the page fetches the current code.
 */
function PreviewFailed({ error }: { error: Error }) {
  const { t } = useTranslation()
  return (
    <Alert
      severity="error"
      action={
        <Button
          color="inherit"
          size="small"
          onClick={() => window.location.reload()}
        >
          {t('attachments.reload')}
        </Button>
      }
    >
      {t('attachments.previewError', { message: error.message })}
    </Alert>
  )
}

function PreviewBody({
  attachment,
  kind,
  fit,
}: {
  attachment: AttachmentMeta
  kind: AttachmentKind
  fit: PreviewFit
}) {
  if (kind === 'image') return <ImageView attachment={attachment} fit={fit} />
  return <TextPreview attachment={attachment} kind={kind} fit={fit} />
}

function ImageView({
  attachment,
  fit,
}: {
  attachment: AttachmentMeta
  fit: PreviewFit
}) {
  const { t } = useTranslation()
  const [failed, setFailed] = useState(false)
  if (failed) {
    return <Alert severity="warning">{t('attachments.imageError')}</Alert>
  }
  return (
    <Box sx={{ display: 'flex', justifyContent: 'center' }}>
      <Box
        component="img"
        src={attachmentContentUrl(attachment.id)}
        alt={attachment.file_name}
        onError={() => setFailed(true)}
        sx={{
          display: 'block',
          maxWidth: '100%',
          maxHeight: fit === 'dialog' ? 'calc(100vh - 220px)' : undefined,
          objectFit: 'contain',
          // A checkerboard shows where the image is transparent.
          backgroundColor: 'action.hover',
          backgroundImage:
            'conic-gradient(rgba(127,127,127,0.12) 25%, transparent 0 50%, rgba(127,127,127,0.12) 0 75%, transparent 0)',
          backgroundSize: '16px 16px',
        }}
      />
    </Box>
  )
}

function TextPreview({
  attachment,
  kind,
  fit,
}: {
  attachment: AttachmentMeta
  kind: Exclude<AttachmentKind, 'image'>
  fit: PreviewFit
}) {
  const { t } = useTranslation()
  const tooLarge = attachment.size_bytes > maxTextPreviewBytes
  const content = useAttachmentText(attachment.id, !tooLarge)

  if (tooLarge) {
    return (
      <Alert severity="info">
        {t('attachments.tooLarge', {
          size: formatBytes(attachment.size_bytes),
          limit: formatBytes(maxTextPreviewBytes),
        })}
      </Alert>
    )
  }
  if (content.isPending) {
    return <LinearProgress aria-label={t('common.loading')} />
  }
  if (content.isError) {
    return (
      <ErrorAlert
        message={t('attachments.loadError', { message: content.error.message })}
        onRetry={() => content.refetch()}
      />
    )
  }
  const text = content.data ?? ''
  if (text.trim() === '') {
    return (
      <Typography color="text.secondary">{t('attachments.empty')}</Typography>
    )
  }
  if (kind === 'markdown') {
    return (
      // On a wide page, keep Markdown lines short enough to read.
      <Box sx={fit === 'page' ? { maxWidth: '80ch' } : undefined}>
        <Suspense fallback={<LinearProgress aria-label={t('common.loading')} />}>
          <MarkdownView text={text} />
        </Suspense>
      </Box>
    )
  }
  if (kind === 'json') return <JsonView text={text} />
  return <TextView text={text} />
}
