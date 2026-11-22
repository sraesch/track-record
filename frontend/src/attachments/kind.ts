import type { AttachmentMeta } from '../api/client'

// How the UI can show an attachment (FR-5). Uploaded files are shown by the
// app itself, never by the browser from the backend's URL (ADR 0003, ADR
// 0013): images in an <img>, text fetched and rendered as React elements.
// Anything else (and HTML in particular) can only be downloaded.

export type AttachmentKind = 'image' | 'markdown' | 'json' | 'text'

/** Image types every current browser shows in an <img>. */
const imageTypes = new Set([
  'image/png',
  'image/jpeg',
  'image/gif',
  'image/webp',
  'image/avif',
  'image/bmp',
  'image/svg+xml',
])

/** Types that say nothing about the content; the extension decides. */
const genericTypes = new Set([
  'application/octet-stream',
  'binary/octet-stream',
  'text/plain',
  '',
])

const byExtension: Record<string, AttachmentKind> = {
  png: 'image',
  jpg: 'image',
  jpeg: 'image',
  gif: 'image',
  webp: 'image',
  avif: 'image',
  bmp: 'image',
  svg: 'image',
  md: 'markdown',
  markdown: 'markdown',
  json: 'json',
  txt: 'text',
  log: 'text',
  csv: 'text',
}

/** The media type without parameters, in lower case: "text/markdown". */
export function mediaType(contentType: string): string {
  return contentType.split(';')[0]!.trim().toLowerCase()
}

/** How an attachment can be shown, or undefined if only as a download. */
export function attachmentKind(
  attachment: Pick<AttachmentMeta, 'content_type' | 'file_name'>,
): AttachmentKind | undefined {
  const type = mediaType(attachment.content_type)
  if (imageTypes.has(type)) return 'image'
  if (type === 'text/markdown' || type === 'text/x-markdown') return 'markdown'
  if (type === 'application/json' || type.endsWith('+json')) return 'json'
  if (genericTypes.has(type)) {
    const extension = attachment.file_name.split('.').at(-1)?.toLowerCase()
    const kind = extension ? byExtension[extension] : undefined
    // A text file is not shown as an image, whatever its name.
    if (kind === 'image' && type === 'text/plain') return 'text'
    if (kind) return kind
    return type === 'text/plain' ? 'text' : undefined
  }
  // Other text, except HTML: uploaded HTML is never rendered (ADR 0003).
  if (type.startsWith('text/') && type !== 'text/html') return 'text'
  return undefined
}

/** Text attachments larger than this are not loaded into the page. */
export const maxTextPreviewBytes = 1_000_000

/** Images larger than this get no thumbnail in lists, only an icon. */
export const maxThumbnailBytes = 5_000_000

/** Where the backend serves the file (always as a download). */
export const attachmentContentUrl = (id: string) =>
  `/ui/api/v1/attachments/${id}/content`
