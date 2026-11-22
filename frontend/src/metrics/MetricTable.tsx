import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward'
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward'
import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown'
import KeyboardArrowUpIcon from '@mui/icons-material/KeyboardArrowUp'
import ListAltIcon from '@mui/icons-material/ListAlt'
import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Collapse from '@mui/material/Collapse'
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
import { type ReactNode, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import type { MetricDirection, MetricVersion, Unit } from '../api/client'
import { formatDateTime, formatTimestamp } from '../format'
import { visuallyHidden } from '../pages/visuallyHidden'
import { changedFields, type ChangedField, type MetricRow } from './metricModel'
import { UnitLabel } from './UnitLabel'

type Props = {
  rows: readonly MetricRow[]
  catalogue: ReadonlyMap<string, Unit> | undefined
}

/** Metrics with their latest definition; older versions on request. */
export function MetricTable({ rows, catalogue }: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState<ReadonlySet<number>>(new Set())
  const toggle = (id: number) =>
    setOpen((current) => {
      const next = new Set(current)
      if (!next.delete(id)) next.add(id)
      return next
    })

  return (
    <TableContainer component={Paper} variant="outlined">
      <Table size="small" aria-label={t('metrics.tableLabel')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('metrics.columns.name')}</TableCell>
            <TableCell>{t('metrics.columns.unit')}</TableCell>
            <TableCell>{t('metrics.columns.valueType')}</TableCell>
            <TableCell>{t('metrics.columns.direction')}</TableCell>
            <TableCell>{t('metrics.columns.description')}</TableCell>
            <TableCell>{t('metrics.columns.version')}</TableCell>
            <TableCell align="right">
              <Box component="span" sx={visuallyHidden}>
                {t('metrics.columns.actions')}
              </Box>
            </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => {
            const expanded = open.has(row.id)
            const older = row.versions.length > 1
            const detailsId = `metric-versions-${row.id}`
            return [
              <TableRow
                key={row.id}
                sx={older && expanded ? { '& > td': { borderBottom: 0 } } : {}}
              >
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Box
                    component="code"
                    sx={{ fontFamily: 'monospace', fontWeight: 500 }}
                  >
                    {row.name}
                  </Box>
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <UnitLabel symbol={row.latest.unit} catalogue={catalogue} />
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  {t(`metrics.valueType.${row.latest.value_type}`)}
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Direction direction={row.latest.direction} />
                </TableCell>
                <TableCell sx={{ minWidth: 200 }}>
                  <Description text={row.latest.description} />
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  {older ? (
                    <Box
                      component="button"
                      type="button"
                      aria-expanded={expanded}
                      aria-controls={detailsId}
                      onClick={() => toggle(row.id)}
                      sx={{
                        all: 'unset',
                        cursor: 'pointer',
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 0.5,
                        color: 'primary.main',
                        borderRadius: 1,
                        '&:focus-visible': {
                          outline: '2px solid',
                          outlineColor: 'primary.main',
                        },
                      }}
                    >
                      {t('metrics.versionOf', {
                        version: row.latest.version,
                        count: row.versions.length,
                      })}
                      {expanded ? (
                        <KeyboardArrowUpIcon fontSize="small" />
                      ) : (
                        <KeyboardArrowDownIcon fontSize="small" />
                      )}
                    </Box>
                  ) : (
                    <Tooltip
                      describeChild
                      title={t('metrics.definedAt', {
                        time: formatTimestamp(row.latest.created_at),
                      })}
                    >
                      <span>
                        {t('metrics.version', { version: row.latest.version })}
                      </span>
                    </Tooltip>
                  )}
                </TableCell>
                <TableCell align="right">
                  <Tooltip title={t('metrics.runs')}>
                    <IconButton
                      size="small"
                      component={RouterLink}
                      to={`/runs?${new URLSearchParams({ metric: row.name })}`}
                      aria-label={t('metrics.runsNamed', { name: row.name })}
                    >
                      <ListAltIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                </TableCell>
              </TableRow>,
              older && (
                <TableRow key={`${row.id}-versions`}>
                  <TableCell
                    colSpan={7}
                    sx={{ py: 0, ...(expanded ? {} : { borderBottom: 0 }) }}
                  >
                    <Collapse in={expanded} unmountOnExit>
                      <Box id={detailsId} sx={{ pb: 2, pl: 2 }}>
                        <VersionTable
                          name={row.name}
                          versions={row.versions}
                          catalogue={catalogue}
                        />
                      </Box>
                    </Collapse>
                  </TableCell>
                </TableRow>
              ),
            ]
          })}
        </TableBody>
      </Table>
    </TableContainer>
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

function Description({ text }: { text: string }) {
  const { t } = useTranslation()
  return text ? (
    text
  ) : (
    <Typography component="span" variant="body2" color="text.secondary">
      {t('common.none')}
    </Typography>
  )
}

/** All versions of one metric, newest first, with what each one changed. */
function VersionTable({
  name,
  versions,
  catalogue,
}: {
  name: string
  versions: readonly MetricVersion[]
  catalogue: ReadonlyMap<string, Unit> | undefined
}) {
  const { t } = useTranslation()
  return (
    <Table size="small" aria-label={t('metrics.versionsLabel', { name })}>
      <TableHead>
        <TableRow>
          <TableCell>{t('metrics.columns.version')}</TableCell>
          <TableCell>{t('metrics.columns.unit')}</TableCell>
          <TableCell>{t('metrics.columns.valueType')}</TableCell>
          <TableCell>{t('metrics.columns.direction')}</TableCell>
          <TableCell>{t('metrics.columns.description')}</TableCell>
          <TableCell>{t('metrics.columns.defined')}</TableCell>
        </TableRow>
      </TableHead>
      <TableBody>
        {versions.map((version, index) => {
          const changed = changedFields(version, versions[index + 1])
          const cell = (field: ChangedField, content: ReactNode) => (
            <TableCell
              sx={{
                whiteSpace: field === 'description' ? undefined : 'nowrap',
              }}
            >
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                <span>{content}</span>
                {changed.includes(field) && (
                  <Chip
                    size="small"
                    color="info"
                    variant="outlined"
                    label={t('metrics.changed')}
                  />
                )}
              </Box>
            </TableCell>
          )
          return (
            <TableRow key={version.id}>
              <TableCell sx={{ whiteSpace: 'nowrap' }}>
                {t('metrics.version', { version: version.version })}
              </TableCell>
              {cell(
                'unit',
                <UnitLabel symbol={version.unit} catalogue={catalogue} />,
              )}
              {cell('value_type', t(`metrics.valueType.${version.value_type}`))}
              {cell('direction', <Direction direction={version.direction} />)}
              {cell('description', <Description text={version.description} />)}
              <TableCell sx={{ whiteSpace: 'nowrap' }}>
                <Tooltip
                  describeChild
                  title={formatTimestamp(version.created_at)}
                >
                  <time dateTime={version.created_at}>
                    {formatDateTime(version.created_at)}
                  </time>
                </Tooltip>
              </TableCell>
            </TableRow>
          )
        })}
      </TableBody>
    </Table>
  )
}
