import Box from '@mui/material/Box'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Typography from '@mui/material/Typography'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { SampleDetail } from '../../api/client'
import { formatNumber } from '../../format'
import { Section } from '../../pages/Section'
import {
  byRepetition,
  type MetricRef,
  useMetricLabel,
  warmupRowSx,
} from './samples'
import { WarmupChip } from './WarmupChip'

type Column = MetricRef & { unit: string }

/**
 * The scalar measurements of every sample as uploaded: one row per sample,
 * one column per metric version. Nothing is aggregated here.
 */
export function SampleMeasurements({
  samples,
}: {
  samples: readonly SampleDetail[]
}) {
  const { t } = useTranslation()
  const rows = useMemo(() => byRepetition(samples), [samples])
  const columns = useMemo(() => {
    const byId = new Map<number, Column>()
    for (const sample of samples) {
      for (const m of sample.measurements) byId.set(m.metric_version_id, m)
    }
    return [...byId.values()].sort(
      (a, b) =>
        a.metric.localeCompare(b.metric) || a.metric_version - b.metric_version,
    )
  }, [samples])
  const label = useMetricLabel(columns)
  const warmup = samples.filter((s) => s.is_warmup).length

  return (
    <Section
      title={t('caseExecution.measurements.title')}
      flush
      actions={
        <Typography variant="body2" color="text.secondary">
          {warmup > 0
            ? t('caseExecution.measurements.countWithWarmup', {
                count: samples.length,
                warmup,
              })
            : t('caseExecution.measurements.count', { count: samples.length })}
        </Typography>
      }
    >
      {columns.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2, pb: 2 }}>
          {t('caseExecution.measurements.empty')}
        </Typography>
      ) : (
        <TableContainer>
          <Table size="small" aria-label={t('caseExecution.measurements.title')}>
            <TableHead>
              <TableRow>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  {t('caseExecution.repetition')}
                </TableCell>
                {columns.map((column) => (
                  <TableCell
                    key={column.metric_version_id}
                    align="right"
                    sx={{ verticalAlign: 'bottom' }}
                  >
                    <Box component="span" sx={{ display: 'block', whiteSpace: 'nowrap' }}>
                      {label(column)}
                    </Box>
                    {/* An empty line without a unit keeps the names aligned. */}
                    <Typography
                      component="span"
                      variant="caption"
                      color="text.secondary"
                      sx={{ display: 'block' }}
                    >
                      {column.unit || '\u00a0'}
                    </Typography>
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {rows.map((sample) => {
                const values = new Map(
                  sample.measurements.map((m) => [m.metric_version_id, m.value]),
                )
                return (
                  <TableRow
                    key={sample.id}
                    sx={sample.is_warmup ? warmupRowSx : undefined}
                  >
                    <TableCell sx={{ whiteSpace: 'nowrap' }}>
                      {sample.repetition}
                      {sample.is_warmup && <WarmupChip />}
                    </TableCell>
                    {columns.map((column) => {
                      const value = values.get(column.metric_version_id)
                      return (
                        <TableCell
                          key={column.metric_version_id}
                          align="right"
                          sx={{ fontVariantNumeric: 'tabular-nums' }}
                        >
                          {value === undefined
                            ? t('common.none')
                            : formatNumber(value)}
                        </TableCell>
                      )
                    })}
                  </TableRow>
                )
              })}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </Section>
  )
}
