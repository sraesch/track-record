import { useQuery } from '@tanstack/react-query'
import { api, unwrap } from '../api/client'

/**
 * The content of a text attachment. Attachments never change once
 * uploaded (ADR 0003), so it is loaded once per page view.
 */
export function useAttachmentText(attachmentId: string, enabled: boolean) {
  return useQuery({
    queryKey: ['attachments', attachmentId, 'content'],
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET('/ui/api/v1/attachments/{attachment_id}/content', {
          params: { path: { attachment_id: attachmentId } },
          parseAs: 'text',
          signal,
        }),
      ),
    enabled,
    staleTime: Infinity,
  })
}
