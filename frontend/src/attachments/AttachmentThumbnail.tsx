import ImageOutlinedIcon from '@mui/icons-material/ImageOutlined'
import Box from '@mui/material/Box'
import { useState } from 'react'
import type { AttachmentMeta } from '../api/client'
import { attachmentContentUrl, maxThumbnailBytes } from './kind'

const size = { width: 56, height: 40 }

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
  const frame = {
    ...size,
    flexShrink: 0,
    borderRadius: 0.5,
    border: 1,
    borderColor: 'divider',
    bgcolor: 'action.hover',
  }
  if (failed || attachment.size_bytes > maxThumbnailBytes) {
    return (
      <Box
        sx={{
          ...frame,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          color: 'text.secondary',
        }}
      >
        <ImageOutlinedIcon fontSize="small" />
      </Box>
    )
  }
  return (
    <Box
      component="img"
      src={attachmentContentUrl(attachment.id)}
      alt=""
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
      sx={{ ...frame, display: 'block', objectFit: 'cover' }}
    />
  )
}
