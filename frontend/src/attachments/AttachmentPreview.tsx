import CloseIcon from '@mui/icons-material/Close'
import DownloadIcon from '@mui/icons-material/Download'
import OpenInNewIcon from '@mui/icons-material/OpenInNew'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogContent from '@mui/material/DialogContent'
import DialogTitle from '@mui/material/DialogTitle'
import IconButton from '@mui/material/IconButton'
import { useTheme } from '@mui/material/styles'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import useMediaQuery from '@mui/material/useMediaQuery'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { AttachmentMeta } from '../api/client'
import { formatBytes } from '../format'
import { attachmentPath } from '../runs/paths'
import { AttachmentView } from './AttachmentView'
import { attachmentContentUrl, attachmentKind, mediaType } from './kind'

type Props = {
  /** The attachment to show; the dialog is closed while there is none. */
  attachment: AttachmentMeta | undefined
  onClose: () => void
}

/**
 * An attachment in a dialog over its run's page. It can also be opened on
 * its own page in a new tab, and downloaded.
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
            <Tooltip title={t('attachments.openInNewTab')}>
              <IconButton
                href={attachmentPath(shown.run_id, shown.id)}
                target="_blank"
                rel="noopener"
                aria-label={t('attachments.openInNewTab')}
              >
                <OpenInNewIcon />
              </IconButton>
            </Tooltip>
            <IconButton aria-label={t('attachments.close')} onClick={onClose}>
              <CloseIcon />
            </IconButton>
          </Box>
          <DialogContent dividers>
            <AttachmentView attachment={shown} fit="dialog" />
          </DialogContent>
        </>
      )}
    </Dialog>
  )
}
