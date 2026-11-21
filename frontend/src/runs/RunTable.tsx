import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Link from '@mui/material/Link'
import Paper from '@mui/material/Paper'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useNavigate } from 'react-router'
import type { RunSummary } from '../api/client'
import {
  formatDateTime,
  formatDuration,
  formatTimestamp,
  shortCommit,
} from '../format'
import { InvalidChip, RunStatusChip } from './RunStatusChip'

type Props = {
  runs: readonly RunSummary[]
  /** Called when a tag chip is clicked, to filter by that tag. */
  onTagClick: (tag: string) => void
}

/** Runs as a table; a row opens the run (FR-21: /runs/:runId). */
export function RunTable({ runs, onTagClick }: Props) {
  const { t } = useTranslation()
  const navigate = useNavigate()

  return (
    <TableContainer component={Paper} variant="outlined">
      <Table size="small" aria-label={t('nav.items.runs')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('runs.columns.status')}</TableCell>
            <TableCell>{t('runs.columns.started')}</TableCell>
            <TableCell>{t('runs.columns.duration')}</TableCell>
            <TableCell>{t('runs.columns.suite')}</TableCell>
            <TableCell>{t('runs.columns.sutVersion')}</TableCell>
            <TableCell>{t('runs.columns.trigger')}</TableCell>
            <TableCell>{t('runs.columns.tags')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {runs.map((run) => {
            const href = `/runs/${run.id}`
            const started = formatDateTime(run.started_at)
            return (
              <TableRow
                key={run.id}
                hover
                onClick={(event) => {
                  // Let links and chips inside the row do their own thing.
                  const target = event.target as HTMLElement
                  if (target.closest('a, button, [role="button"]')) return
                  // Modified clicks (new tab, …) are left to the run link.
                  if (
                    event.button !== 0 ||
                    event.metaKey ||
                    event.ctrlKey ||
                    event.shiftKey ||
                    event.altKey
                  )
                    return
                  // Selecting text (e.g. to copy a commit) is not a click.
                  if (window.getSelection()?.toString()) return
                  navigate(href)
                }}
                sx={{
                  cursor: 'pointer',
                  opacity: run.is_valid ? 1 : 0.6,
                  '& td': { whiteSpace: 'nowrap' },
                }}
              >
                <TableCell>
                  <Box sx={{ display: 'flex', gap: 0.5 }}>
                    <RunStatusChip status={run.status} />
                    {!run.is_valid && (
                      <InvalidChip reason={run.invalid_reason} />
                    )}
                  </Box>
                </TableCell>
                <TableCell>
                  <Tooltip title={formatTimestamp(run.started_at)}>
                    <Link
                      component={RouterLink}
                      to={href}
                      underline="hover"
                      aria-label={t('runs.open', { started })}
                    >
                      {started}
                    </Link>
                  </Tooltip>
                </TableCell>
                <TableCell>
                  {run.ended_at
                    ? formatDuration(run.started_at, run.ended_at)
                    : run.status === 'running'
                      ? t('runs.stillRunning')
                      : t('common.none')}
                </TableCell>
                <TableCell>
                  {run.suite.name}
                  <Typography
                    component="span"
                    variant="body2"
                    color="text.secondary"
                    sx={{ ml: 0.5 }}
                  >
                    {t('runs.version', { version: run.suite.version })}
                  </Typography>
                </TableCell>
                <TableCell>
                  <SutVersion version={run.sut_version} />
                </TableCell>
                <TableCell>{t(`runs.trigger.${run.triggered_by}`)}</TableCell>
                <TableCell>
                  <Box sx={{ display: 'flex', gap: 0.5 }}>
                    {run.tags.map((tag) => (
                      <Chip
                        key={tag}
                        size="small"
                        label={tag}
                        aria-label={t('runs.filterByTag', { tag })}
                        onClick={(event) => {
                          event.stopPropagation()
                          onTagClick(tag)
                        }}
                      />
                    ))}
                  </Box>
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </TableContainer>
  )
}

function SutVersion({ version }: { version: RunSummary['sut_version'] }) {
  return (
    <Box sx={{ display: 'flex', alignItems: 'baseline', gap: 1 }}>
      {version.branch && <span>{version.branch}</span>}
      <Tooltip title={version.commit}>
        <Typography
          component="code"
          variant="body2"
          color="text.secondary"
          sx={{ fontFamily: 'monospace' }}
        >
          {shortCommit(version.commit)}
        </Typography>
      </Tooltip>
      {version.tag && (
        <Chip size="small" variant="outlined" label={version.tag} />
      )}
      {version.build_config && (
        <Typography component="span" variant="body2" color="text.secondary">
          {version.build_config}
        </Typography>
      )}
    </Box>
  )
}
