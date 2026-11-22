import DownloadIcon from '@mui/icons-material/Download'
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
import { formatBytes, formatDateTime, formatTimestamp } from '../../format'
import { Section } from '../../pages/Section'
import { caseExecutionPath } from '../paths'

type Props = {
  attachments: readonly AttachmentMeta[]
  /** The run's case executions by ID, to name what a file is attached to. */
  caseExecutions: ReadonlyMap<string, CaseExecutionSummary>
}

/**
 * Files uploaded with a run (FR-5): logs, screenshots, traces. They are
 * always downloaded, never shown inline (the backend sends them as
 * attachments).
 */
export function RunAttachments({ attachments, caseExecutions }: Props) {
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
            <TableHead>
              <TableRow>
                <TableCell>{t('runDetail.attachments.file')}</TableCell>
                <TableCell>{t('runDetail.attachments.attachedTo')}</TableCell>
                <TableCell>{t('runDetail.attachments.type')}</TableCell>
                <TableCell align="right">
                  {t('runDetail.attachments.size')}
                </TableCell>
                <TableCell>{t('runDetail.attachments.uploaded')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {attachments.map((attachment) => (
                <TableRow key={attachment.id}>
                  <TableCell sx={{ overflowWrap: 'anywhere' }}>
                    <Link
                      href={`/ui/api/v1/attachments/${attachment.id}/content`}
                      download={attachment.file_name}
                      underline="hover"
                      aria-label={t('runDetail.attachments.download', {
                        file: attachment.file_name,
                      })}
                      sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.5 }}
                    >
                      <DownloadIcon fontSize="small" />
                      {attachment.file_name}
                    </Link>
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
                      {attachment.content_type}
                    </Typography>
                  </TableCell>
                  <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                    {formatBytes(attachment.size_bytes)}
                  </TableCell>
                  <TableCell sx={{ whiteSpace: 'nowrap' }}>
                    <Tooltip describeChild title={formatTimestamp(attachment.created_at)}>
                      <time dateTime={attachment.created_at}>
                        {formatDateTime(attachment.created_at)}
                      </time>
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
