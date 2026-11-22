import ErrorOutlineIcon from '@mui/icons-material/ErrorOutlineOutlined'
import Box from '@mui/material/Box'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { type ReactNode, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { EvaluateResponse, SampleResult } from '../api/client'
import { Chart } from '../charts/Chart'
import { useChartColors } from '../charts/colors'
import { formatNumber, formatRounded } from '../format'
import { WarmupChip } from '../runs/case/WarmupChip'
import { warmupRowSx } from '../runs/case/samples'
import { type ChartLine, seriesChartOption } from '../runs/case/seriesChart'

// Values of derived metrics per sample, as the editor's preview and the
// case execution page show them. A derived metric can have no value, which
// is not missing data (FR-12a); a sample without the inputs of the
// expression is missing; a data error names its cause.

/** One sample's result: the value, or what took its place. */
export function SampleValue({ result }: { result: SampleResult }) {
  const { t } = useTranslation()
  if (result.status === 'value') {
    if (result.value === null || result.value === undefined) {
      return t('common.none')
    }
    // Computed values carry every digit of a double; the full one on hover.
    return (
      <span title={formatNumber(result.value)}>
        {formatRounded(result.value)}
      </span>
    )
  }
  const hint =
    result.status === 'no_value'
      ? t('derived.status.noValueHint')
      : (result.message ?? '')
  return (
    <Tooltip title={hint} describeChild>
      <Box
        component="span"
        tabIndex={0}
        sx={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 0.5,
          fontStyle: 'italic',
          color: result.status === 'error' ? 'error.main' : 'text.secondary',
        }}
      >
        {result.status === 'error' && (
          <ErrorOutlineIcon fontSize="inherit" aria-hidden />
        )}
        {t(`derived.status.${result.status}`)}
      </Box>
    </Tooltip>
  )
}

export type SampleRow = { id: string; repetition: number; is_warmup: boolean }

export type ValueColumn = {
  key: string
  /** Header: the metric's name, maybe as a link. */
  label: ReactNode
  unit: string
  /** By sample id. */
  results: ReadonlyMap<string, SampleResult>
}

/** Scalar results: one row per sample, one column per expression. */
export function ScalarTable({
  label,
  samples,
  columns,
}: {
  label: string
  samples: readonly SampleRow[]
  columns: readonly ValueColumn[]
}) {
  const { t } = useTranslation()
  const rows = useMemo(
    () => [...samples].sort((a, b) => a.repetition - b.repetition),
    [samples],
  )
  return (
    <TableContainer>
      <Table size="small" aria-label={label}>
        <TableHead>
          <TableRow>
            <TableCell sx={{ whiteSpace: 'nowrap' }}>
              {t('caseExecution.repetition')}
            </TableCell>
            {columns.map((column) => (
              <TableCell
                key={column.key}
                align="right"
                sx={{ verticalAlign: 'bottom' }}
              >
                <Box
                  component="span"
                  sx={{ display: 'block', whiteSpace: 'nowrap' }}
                >
                  {column.label}
                </Box>
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ display: 'block' }}
                >
                  {column.unit || ' '}
                </Typography>
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((sample) => (
            <TableRow
              key={sample.id}
              sx={sample.is_warmup ? warmupRowSx : undefined}
            >
              <TableCell sx={{ whiteSpace: 'nowrap' }}>
                {sample.repetition}
                {sample.is_warmup && <WarmupChip />}
              </TableCell>
              {columns.map((column) => {
                const result = column.results.get(sample.id)
                return (
                  <TableCell
                    key={column.key}
                    align="right"
                    sx={{ fontVariantNumeric: 'tabular-nums' }}
                  >
                    {result ? <SampleValue result={result} /> : t('common.none')}
                  </TableCell>
                )
              })}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  )
}

const chartHeight = 300

/**
 * A series result as a chart: one line per sample, in the colors the
 * case execution's own series charts give the samples. Samples without a
 * series are listed below it with what took its place.
 */
export function SeriesResultChart({
  title,
  response,
}: {
  title: string
  response: EvaluateResponse
}) {
  const { t } = useTranslation()
  const colors = useChartColors()
  const samples = useMemo(
    () => [...response.samples].sort((a, b) => a.repetition - b.repetition),
    [response.samples],
  )
  const xDimension = response.x_dimension ?? ''
  const xUnit = response.x_unit ?? ''
  const xName = xUnit
    ? t('caseExecution.series.xWithUnit', { dimension: xDimension, unit: xUnit })
    : xDimension

  const option = useMemo(() => {
    const withUnit = (unit: string) => (value: number) =>
      unit
        ? t('common.valueWithUnit', { value: formatNumber(value), unit })
        : formatNumber(value)
    const lines = samples.flatMap((sample, colorIndex): ChartLine[] =>
      sample.series
        ? [
            {
              name: sample.is_warmup
                ? t('caseExecution.series.lineWarmup', {
                    repetition: sample.repetition,
                  })
                : t('caseExecution.series.line', {
                    repetition: sample.repetition,
                  }),
              colorIndex,
              warmup: sample.is_warmup,
              x: sample.series.x,
              y: sample.series.y,
            },
          ]
        : [],
    )
    return seriesChartOption({
      lines,
      xName,
      yName: response.unit,
      formatTick: formatNumber,
      formatX: withUnit(xUnit),
      formatY: withUnit(response.unit),
      colors,
    })
  }, [samples, xName, xUnit, response.unit, colors, t])

  const drawn = samples.filter((s) => s.series).length
  const without = samples.filter((s) => !s.series)

  return (
    <Box component="figure" sx={{ m: 0, minWidth: 0 }}>
      <Typography component="figcaption" variant="subtitle2" sx={{ mb: 1 }}>
        {title}
      </Typography>
      {drawn > 0 && (
        <Chart
          option={option}
          height={chartHeight}
          label={t('caseExecution.series.chartLabel', {
            metric: title,
            x: xName,
            count: drawn,
          })}
        />
      )}
      {without.length > 0 && (
        <Box
          component="ul"
          sx={{ m: 0, mt: 1, pl: 0, listStyle: 'none', display: 'grid', gap: 0.5 }}
        >
          {without.map((sample) => (
            <Typography component="li" variant="body2" key={sample.sample_id}>
              {t('derived.sampleWithout', { repetition: sample.repetition })}{' '}
              <SampleValue result={sample} />
            </Typography>
          ))}
        </Box>
      )}
    </Box>
  )
}
