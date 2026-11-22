import { useLocation, useNavigate, useSearchParams } from 'react-router'
import type { AttachmentMeta } from '../api/client'

/** URL query parameter of the attachment shown in the preview dialog. */
export const attachmentParam = 'attachment'

/**
 * The attachment open in the preview dialog of a page. It is in the URL
 * (`?attachment=`, FR-21), next to the page's other view state.
 */
export function useAttachmentPreview(attachments: readonly AttachmentMeta[]) {
  const [searchParams, setSearchParams] = useSearchParams()
  const location = useLocation()
  const navigate = useNavigate()
  const attachmentId = searchParams.get(attachmentParam)

  /** This page's URL with the attachment's preview open. */
  const previewHref = (id: string) => {
    const params = new URLSearchParams(searchParams)
    params.set(attachmentParam, id)
    return `?${params}`
  }
  const closePreview = () => {
    // Opened from the list: going back closes it, as the back button does.
    if ((location.state as { preview?: boolean } | null)?.preview) {
      navigate(-1)
      return
    }
    const params = new URLSearchParams(searchParams)
    params.delete(attachmentParam)
    setSearchParams(params, { replace: true })
  }

  return {
    attachmentId,
    previewed: attachments.find((a) => a.id === attachmentId),
    previewHref,
    closePreview,
  }
}
