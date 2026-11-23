import { Image } from 'lucide-react'
import { useState } from 'react'
import type { AttachmentMeta } from '../api/client'
import { attachmentContentUrl, maxThumbnailBytes } from './kind'
import './attachments.css'

/**
 * A small preview of an image attachment, loaded once it scrolls into
 * view. Very large images (and ones that fail to load) get an icon.
 * Decorative: the link around it is named after the file.
 */
export function AttachmentThumbnail({
  attachment,
}: {
  attachment: AttachmentMeta
}) {
  const [failed, setFailed] = useState(false)
  if (failed || attachment.size_bytes > maxThumbnailBytes) {
    return (
      <div className="tr-attachments-thumb">
        <Image aria-hidden />
      </div>
    )
  }
  return (
    <img
      className="tr-attachments-thumb"
      src={attachmentContentUrl(attachment.id)}
      alt=""
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
    />
  )
}
