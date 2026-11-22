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
import { byRepetition, warmupRowSx } from './samples'
import { SeriesCharts } from './SeriesCharts'
import { WarmupChip } from './WarmupChip'

/**
 * The series of every sample (at most one per metric and sample): a chart
 * per metric with a line per sample, then what is measured over which x
 * dimension, in which units, and how many points.
 */
export function SampleSeries({
  runId,
  samples,
}: {
  runId: string
  samples: readonly SampleDetail[]
}) {
  const { t } = useTranslation()
  const rows = useMemo(
    () =>
      byRepetition(samples).flatMap((sample) =>
        [...sample.series]
          .sort((a, b) => a.metric.localeCompare(b.metric))
          .map((series) => ({ sample, series })),
      ),
    [samples],
  )

  return (
    <Section
      title={t('caseExecution.series.title')}
      flush
      actions={
        <Typography variant="body2" color="text.secondary">
          {t('caseExecution.series.count', { count: rows.length })}
        </Typography>
      }
    >
      {rows.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2, pb: 2 }}>
          {t('caseExecution.series.empty')}
        </Typography>
      ) : (
        <>
          <SeriesCharts runId={runId} samples={samples} />
          <TableContainer>
            <Table size="small" aria-label={t('caseExecution.series.title')}>
              <TableHead sx={{ '& th': { whiteSpace: 'nowrap' } }}>
                <TableRow>
                  <TableCell>{t('caseExecution.repetition')}</TableCell>
                  <TableCell>{t('caseExecution.series.metric')}</TableCell>
                  <TableCell>{t('caseExecution.series.unit')}</TableCell>
                  <TableCell>{t('caseExecution.series.x')}</TableCell>
                  <TableCell align="right">
                    {t('caseExecution.series.points')}
                  </TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {rows.map(({ sample, series }) => (
                  <TableRow
                    key={series.id}
                    sx={sample.is_warmup ? warmupRowSx : undefined}
                  >
                    <TableCell sx={{ whiteSpace: 'nowrap' }}>
                      {sample.repetition}
                      {sample.is_warmup && <WarmupChip />}
                    </TableCell>
                    <TableCell>{series.metric}</TableCell>
                    <TableCell>{series.unit || t('common.none')}</TableCell>
                    <TableCell>
                      {series.x_unit
                        ? t('caseExecution.series.xWithUnit', {
                            dimension: series.x_dimension,
                            unit: series.x_unit,
                          })
                        : series.x_dimension}
                    </TableCell>
                    <TableCell align="right">
                      {formatNumber(series.point_count)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </>
      )}
    </Section>
  )
}
