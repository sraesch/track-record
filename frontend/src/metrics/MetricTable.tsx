import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward'
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward'
import DeleteIcon from '@mui/icons-material/DeleteOutlined'
import RenameIcon from '@mui/icons-material/DriveFileRenameOutline'
import EditIcon from '@mui/icons-material/EditOutlined'
import ListAltIcon from '@mui/icons-material/ListAlt'
import Box from '@mui/material/Box'
import IconButton from '@mui/material/IconButton'
import Paper from '@mui/material/Paper'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import type { Metric, MetricDirection, Unit } from '../api/client'
import { formatDateTime, formatTimestamp } from '../format'
import { visuallyHidden } from '../pages/visuallyHidden'
import { type MetricAction, MetricDialog } from './MetricDialogs'
import { UnitLabel } from './UnitLabel'

type Props = {
  /** The metrics to show. */
  rows: readonly Metric[]
  /** All metrics, shown or not. */
  metrics: readonly Metric[]
  units: readonly Unit[]
  catalogue: ReadonlyMap<string, Unit> | undefined
}

/**
 * The uploaded metrics with their definition. A metric has one definition
 * and no versions (ADR 0018); it is changed, renamed and deleted here.
 */
export function MetricTable({ rows, metrics, units, catalogue }: Props) {
  const { t } = useTranslation()
  // The dialog keeps its metric while it closes, also when the list has
  // changed underneath it (renamed or deleted).
  const [dialog, setDialog] = useState<{
    action: MetricAction
    metric: Metric
  }>()
  const [open, setOpen] = useState(false)
  const start = (action: MetricAction, metric: Metric) => {
    setDialog({ action, metric })
    setOpen(true)
  }

  return (
    <>
      <TableContainer component={Paper} variant="outlined">
        <Table size="small" aria-label={t('metrics.tableLabel')}>
          <TableHead>
            <TableRow>
              <TableCell>{t('metrics.columns.name')}</TableCell>
              <TableCell>{t('metrics.columns.unit')}</TableCell>
              <TableCell>{t('metrics.columns.valueType')}</TableCell>
              <TableCell>{t('metrics.columns.direction')}</TableCell>
              <TableCell>{t('metrics.columns.description')}</TableCell>
              <TableCell>{t('metrics.columns.changed')}</TableCell>
              <TableCell align="right">
                <Box component="span" sx={visuallyHidden}>
                  {t('metrics.columns.actions')}
                </Box>
              </TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((metric) => (
              <TableRow key={metric.id}>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Box
                    component="code"
                    sx={{ fontFamily: 'monospace', fontWeight: 500 }}
                  >
                    {metric.name}
                  </Box>
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <UnitLabel symbol={metric.unit} catalogue={catalogue} />
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  {t(`metrics.valueType.${metric.value_type}`)}
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Direction direction={metric.direction} />
                </TableCell>
                <TableCell sx={{ minWidth: 200 }}>
                  {metric.description || (
                    <Typography
                      component="span"
                      variant="body2"
                      color="text.secondary"
                    >
                      {t('common.none')}
                    </Typography>
                  )}
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Tooltip
                    describeChild
                    title={formatTimestamp(metric.updated_at)}
                  >
                    <time dateTime={metric.updated_at}>
                      {formatDateTime(metric.updated_at)}
                    </time>
                  </Tooltip>
                </TableCell>
                <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                  <Tooltip title={t('metrics.runs')}>
                    <IconButton
                      size="small"
                      component={RouterLink}
                      to={`/runs?${new URLSearchParams({ metric: metric.name })}`}
                      aria-label={t('metrics.runsNamed', { name: metric.name })}
                    >
                      <ListAltIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                  <Tooltip title={t('metrics.edit.action')}>
                    <IconButton
                      size="small"
                      onClick={() => start('edit', metric)}
                      aria-label={t('metrics.edit.named', {
                        name: metric.name,
                      })}
                    >
                      <EditIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                  <Tooltip title={t('metrics.rename.action')}>
                    <IconButton
                      size="small"
                      onClick={() => start('rename', metric)}
                      aria-label={t('metrics.rename.named', {
                        name: metric.name,
                      })}
                    >
                      <RenameIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                  <Tooltip title={t('metrics.delete.action')}>
                    <IconButton
                      size="small"
                      onClick={() => start('delete', metric)}
                      aria-label={t('metrics.delete.named', {
                        name: metric.name,
                      })}
                    >
                      <DeleteIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableContainer>
      {dialog && (
        <MetricDialog
          action={dialog.action}
          metric={dialog.metric}
          metrics={metrics}
          units={units}
          open={open}
          onClose={() => setOpen(false)}
          onExited={() => setDialog(undefined)}
        />
      )}
    </>
  )
}

function Direction({ direction }: { direction: MetricDirection }) {
  const { t } = useTranslation()
  const Icon =
    direction === 'higher_is_better' ? ArrowUpwardIcon : ArrowDownwardIcon
  return (
    <Box sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.5 }}>
      <Icon fontSize="small" sx={{ color: 'text.secondary' }} />
      {t(`metrics.direction.${direction}`)}
    </Box>
  )
}
