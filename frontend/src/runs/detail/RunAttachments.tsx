import DownloadIcon from '@mui/icons-material/Download'
import OpenInNewIcon from '@mui/icons-material/OpenInNew'
import Box from '@mui/material/Box'
import IconButton from '@mui/material/IconButton'
import Link from '@mui/material/Link'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import type { MouseEvent, ReactNode } from 'react'
import { Link as RouterLink, useNavigate } from 'react-router'
import type { AttachmentMeta, CaseExecutionSummary } from '../../api/client'
import { AttachmentThumbnail } from '../../attachments/AttachmentThumbnail'
import {
  attachmentContentUrl,
  attachmentKind,
  mediaType,
} from '../../attachments/kind'
import { formatBytes, formatDateTime, formatTimestamp } from '../../format'
import { Section } from '../../pages/Section'
import { visuallyHidden } from '../../pages/visuallyHidden'
import { attachmentPath, caseExecutionPath } from '../paths'

type Props = {
  attachments: readonly AttachmentMeta[]
  /** What a file is attached to: the run, a case execution or a sample. */
  attachedTo: (attachment: AttachmentMeta) => ReactNode
  /** The URL of this page with an attachment's preview dialog open. */
  previewHref: (attachmentId: string) => string
}

/**
 * Files uploaded with a run (FR-5): logs, screenshots, traces. Images,
 * Markdown, JSON and text open in a preview dialog or, in a new tab, on
 * their own page (images also show a thumbnail here); every file can be
 * downloaded.
 */
export function RunAttachments({
  attachments,
  attachedTo,
  previewHref,
}: Props) {
  const { t } = useTranslation()
  return (
    <Section title={t('runDetail.attachments.title')} flush>
      {attachments.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2, pb: 2 }}>
          {t('runDetail.attachments.empty')}
        </Typography>
      ) : (
        <TableContainer>
          <Table size="small" aria-label={t('runDetail.attachments.title')}>
            <TableHead sx={{ '& th': { whiteSpace: 'nowrap' } }}>
              <TableRow>
                <TableCell>{t('runDetail.attachments.file')}</TableCell>
                <TableCell>{t('runDetail.attachments.attachedTo')}</TableCell>
                <TableCell>{t('runDetail.attachments.type')}</TableCell>
                <TableCell align="right">
                  {t('runDetail.attachments.size')}
                </TableCell>
                <TableCell>{t('runDetail.attachments.uploaded')}</TableCell>
                <TableCell padding="checkbox">
                  <Box component="span" sx={visuallyHidden}>
                    {t('runDetail.attachments.actions')}
                  </Box>
                </TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {attachments.map((attachment) => (
                <TableRow key={attachment.id}>
                  {/* Names break between words, and only inside a word
                      longer than the column. */}
                  <TableCell sx={{ overflowWrap: 'break-word', minWidth: 160 }}>
                    <FileName
                      attachment={attachment}
                      previewHref={previewHref(attachment.id)}
                    />
                  </TableCell>
                  <TableCell>{attachedTo(attachment)}</TableCell>
                  <TableCell>
                    <Typography
                      component="code"
                      variant="body2"
                      sx={{ fontFamily: 'monospace' }}
                    >
                      {mediaType(attachment.content_type)}
                    </Typography>
                  </TableCell>
                  <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                    {formatBytes(attachment.size_bytes)}
                  </TableCell>
                  <TableCell sx={{ whiteSpace: 'nowrap' }}>
                    <Tooltip
                      describeChild
                      title={formatTimestamp(attachment.created_at)}
                    >
                      <time dateTime={attachment.created_at}>
                        {formatDateTime(attachment.created_at)}
                      </time>
                    </Tooltip>
                  </TableCell>
                  <TableCell padding="checkbox" sx={{ whiteSpace: 'nowrap' }}>
                    {attachmentKind(attachment) && (
                      <Tooltip title={t('attachments.openInNewTab')}>
                        <IconButton
                          href={attachmentPath(attachment.run_id, attachment.id)}
                          target="_blank"
                          rel="noopener"
                          size="small"
                          aria-label={t('runDetail.attachments.openInNewTab', {
                            file: attachment.file_name,
                          })}
                        >
                          <OpenInNewIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                    )}
                    <Tooltip title={t('attachments.download')}>
                      <IconButton
                        href={attachmentContentUrl(attachment.id)}
                        download={attachment.file_name}
                        size="small"
                        aria-label={t('runDetail.attachments.download', {
                          file: attachment.file_name,
                        })}
                      >
                        <DownloadIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </Section>
  )
}

/**
 * The file name, opening the preview where there is one (with a thumbnail
 * for images); otherwise plain text, the file is only downloaded. A plain
 * click opens the dialog; the link itself leads to the attachment's own
 * page, so opening it in a new tab (middle click, Ctrl/Cmd+click, the
 * context menu) shows the file on its own.
 */
function FileName({
  attachment,
  previewHref,
}: {
  attachment: AttachmentMeta
  previewHref: string
}) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const kind = attachmentKind(attachment)
  if (!kind) return attachment.file_name
  return (
    <Link
      component={RouterLink}
      to={attachmentPath(attachment.run_id, attachment.id)}
      onClick={(event: MouseEvent) => {
        const modified =
          event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
        if (event.button !== 0 || modified) return
        event.preventDefault()
        navigate(previewHref, { state: { preview: true } })
      }}
      underline="hover"
      aria-label={t('runDetail.attachments.open', {
        file: attachment.file_name,
      })}
      sx={{ display: 'inline-flex', alignItems: 'center', gap: 1.5 }}
    >
      {kind === 'image' && <AttachmentThumbnail attachment={attachment} />}
      {attachment.file_name}
    </Link>
  )
}

/** The run, a case execution or one of its samples. */
export function AttachedTo({
  attachment,
  caseExecutions,
}: {
  attachment: AttachmentMeta
  caseExecutions: ReadonlyMap<string, CaseExecutionSummary>
}) {
  const { t } = useTranslation()
  const id = attachment.case_execution_id
  if (!id) return t('runDetail.attachments.toRun')

  const ce = caseExecutions.get(id)
  const name = ce
    ? t('runDetail.attachments.caseName', {
        scenario: ce.test_case.scenario,
        device: ce.client_device.resolved_name,
      })
    : t('runDetail.attachments.caseUnknown')
  return (
    <Link
      component={RouterLink}
      to={caseExecutionPath(attachment.run_id, id)}
      underline="hover"
    >
      {attachment.sample_id
        ? t('runDetail.attachments.toSample', { name })
        : name}
    </Link>
  )
}
