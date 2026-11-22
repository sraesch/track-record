import CloseIcon from '@mui/icons-material/Close'
import DownloadIcon from '@mui/icons-material/Download'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogContent from '@mui/material/DialogContent'
import DialogTitle from '@mui/material/DialogTitle'
import IconButton from '@mui/material/IconButton'
import LinearProgress from '@mui/material/LinearProgress'
import { useTheme } from '@mui/material/styles'
import Typography from '@mui/material/Typography'
import useMediaQuery from '@mui/material/useMediaQuery'
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
  mediaType,
} from './kind'
import { useAttachmentText } from './queries'
import { TextView } from './TextView'

// The Markdown renderer is large and rarely needed: it is loaded when the
// first Markdown attachment is opened (own chunk, see vite.config.ts).
const MarkdownView = lazy(() =>
  import('./MarkdownView').then((module) => ({ default: module.MarkdownView })),
)

type Props = {
  /** The attachment to show; the dialog is closed while there is none. */
  attachment: AttachmentMeta | undefined
  onClose: () => void
}

/**
 * Shows an attachment in the app: images as they are, Markdown formatted,
 * JSON highlighted, other text as it is. It can always be downloaded too.
 */
export function AttachmentPreview({ attachment, onClose }: Props) {
  const { t } = useTranslation()
  const theme = useTheme()
  const fullScreen = useMediaQuery(theme.breakpoints.down('sm'))
  // Keep showing the last attachment while the dialog fades out.
  const [shown, setShown] = useState(attachment)
  if (attachment && attachment !== shown) setShown(attachment)

  const kind = shown && attachmentKind(shown)
  return (
    <Dialog
      open={attachment !== undefined}
      onClose={onClose}
      fullScreen={fullScreen}
      fullWidth
      maxWidth={kind === 'image' ? 'lg' : 'md'}
    >
      {shown && (
        <>
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              gap: 1,
              pl: 3,
              pr: 1.5,
              py: 1.5,
            }}
          >
            <Box sx={{ flexGrow: 1, minWidth: 0 }}>
              {/* Names the dialog: MUI links it to the dialog's label. */}
              <DialogTitle
                sx={{ p: 0, overflowWrap: 'anywhere', lineHeight: 1.3 }}
              >
                {shown.file_name}
              </DialogTitle>
              <Typography variant="body2" color="text.secondary">
                {t('attachments.info', {
                  type: mediaType(shown.content_type),
                  size: formatBytes(shown.size_bytes),
                })}
              </Typography>
            </Box>
            <Button
              href={attachmentContentUrl(shown.id)}
              download={shown.file_name}
              size="small"
              variant="outlined"
              startIcon={<DownloadIcon />}
              sx={{ flexShrink: 0 }}
            >
              {t('attachments.download')}
            </Button>
            <IconButton aria-label={t('attachments.close')} onClick={onClose}>
              <CloseIcon />
            </IconButton>
          </Box>
          <DialogContent dividers>
            {kind ? (
              <ErrorBoundary
                key={shown.id}
                fallback={(error) => <PreviewFailed error={error} />}
              >
                <PreviewBody attachment={shown} kind={kind} />
              </ErrorBoundary>
            ) : (
              <Typography color="text.secondary">
                {t('attachments.noPreview')}
              </Typography>
            )}
          </DialogContent>
        </>
      )}
    </Dialog>
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
}: {
  attachment: AttachmentMeta
  kind: AttachmentKind
}) {
  if (kind === 'image') return <ImageView attachment={attachment} />
  return <TextPreview attachment={attachment} kind={kind} />
}

function ImageView({ attachment }: { attachment: AttachmentMeta }) {
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
          maxHeight: 'calc(100vh - 220px)',
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
}: {
  attachment: AttachmentMeta
  kind: Exclude<AttachmentKind, 'image'>
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
      <Suspense fallback={<LinearProgress aria-label={t('common.loading')} />}>
        <MarkdownView text={text} />
      </Suspense>
    )
  }
  if (kind === 'json') return <JsonView text={text} />
  return <TextView text={text} />
}
