import { Download, ExternalLink, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { AttachmentMeta } from '../api/client'
import { formatBytes } from '../format'
import { attachmentPath } from '../runs/paths'
import {
  AnchorButton,
  breakpoints,
  Dialog,
  IconButton,
  IconLinkButton,
  useMediaQuery,
} from '../ui'
import { AttachmentView } from './AttachmentView'
import { attachmentContentUrl, attachmentKind, mediaType } from './kind'
import './attachments.css'

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
  const fullScreen = !useMediaQuery(`(min-width:${breakpoints.sm}px)`)
  const kind = attachment && attachmentKind(attachment)
  return (
    <Dialog
      open={attachment !== undefined}
      onClose={onClose}
      // The file name names the dialog.
      title={attachment?.file_name}
      size={fullScreen ? 'full' : kind === 'image' ? 'lg' : 'md'}
      titleActions={
        attachment && (
          <>
            <AnchorButton
              size="sm"
              icon={Download}
              href={attachmentContentUrl(attachment.id)}
              download={attachment.file_name}
            >
              {t('attachments.download')}
            </AnchorButton>
            <IconLinkButton
              icon={ExternalLink}
              label={t('attachments.openInNewTab')}
              to={attachmentPath(attachment.run_id, attachment.id)}
              target="_blank"
              rel="noopener"
            />
            <IconButton
              icon={X}
              label={t('attachments.close')}
              onClick={onClose}
            />
          </>
        )
      }
    >
      {attachment && (
        <>
          <p className="ui-text small ui-secondary">
            {t('attachments.info', {
              type: mediaType(attachment.content_type),
              size: formatBytes(attachment.size_bytes),
            })}
          </p>
          <AttachmentView attachment={attachment} fit="dialog" />
        </>
      )}
    </Dialog>
  )
}
