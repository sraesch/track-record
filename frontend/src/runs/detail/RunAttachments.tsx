import DownloadIcon from '@mui/icons-material/Download'
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
import { Link as RouterLink } from 'react-router'
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
import { caseExecutionPath } from '../paths'

type Props = {
  attachments: readonly AttachmentMeta[]
  /** The run's case executions by ID, to name what a file is attached to. */
  caseExecutions: ReadonlyMap<string, CaseExecutionSummary>
  /** The URL that opens an attachment's preview on this page. */
  previewHref: (attachmentId: string) => string
}

/**
 * Files uploaded with a run (FR-5): logs, screenshots, traces. Images,
 * Markdown, JSON and text open in a preview (images also show a thumbnail
 * here); every file can be downloaded.
 */
export function RunAttachments({
  attachments,
  caseExecutions,
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
                      href={previewHref(attachment.id)}
                    />
                  </TableCell>
                  <TableCell>
                    <AttachedTo
                      attachment={attachment}
                      caseExecutions={caseExecutions}
                    />
                  </TableCell>
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
                  <TableCell padding="checkbox">
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
 * for images); otherwise plain text, the file is only downloaded.
 */
function FileName({
  attachment,
  href,
}: {
  attachment: AttachmentMeta
  href: string
}) {
  const { t } = useTranslation()
  const kind = attachmentKind(attachment)
  if (!kind) return attachment.file_name
  return (
    <Link
      component={RouterLink}
      to={href}
      state={{ preview: true }}
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
function AttachedTo({
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
