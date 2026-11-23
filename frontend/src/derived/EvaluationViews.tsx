import { CircleAlert } from 'lucide-react'
import { type ReactNode, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { EvaluateResponse, SampleResult } from '../api/client'
import { Chart } from '../charts/Chart'
import { useChartColors } from '../charts/colors'
import { formatNumber, formatRounded } from '../format'
import { WarmupChip } from '../runs/case/WarmupChip'
import { type ChartLine, seriesChartOption } from '../runs/case/seriesChart'
import { Table, Tooltip } from '../ui'
import './derived.css'

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
  const error = result.status === 'error'
  return (
    <Tooltip title={hint} describes>
      <span
        // Focusable, so the hint also shows for the keyboard.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        className={
          error ? 'tr-derived-status tr-derived-error' : 'tr-derived-status'
        }
      >
        {error && <CircleAlert size={12} aria-hidden />}
        {t(`derived.status.${result.status}`)}
      </span>
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
  framed = false,
}: {
  label: string
  samples: readonly SampleRow[]
  columns: readonly ValueColumn[]
  /** A border around the table; leave it out inside a flush section. */
  framed?: boolean
}) {
  const { t } = useTranslation()
  const rows = useMemo(
    () => [...samples].sort((a, b) => a.repetition - b.repetition),
    [samples],
  )
  return (
    <Table framed={framed} aria-label={label}>
      <thead>
        <tr>
          <th className="tr-derived-value-head">
            {t('caseExecution.repetition')}
          </th>
          {columns.map((column) => (
            <th key={column.key} className="g-r tr-derived-value-head">
              <span>{column.label}</span>
              <span className="tr-derived-value-unit">
                {column.unit || '\u00a0'}
              </span>
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((sample) => (
          <tr
            key={sample.id}
            className={sample.is_warmup ? 'tr-derived-warmup' : undefined}
          >
            <td>
              {sample.repetition}
              {sample.is_warmup && <WarmupChip />}
            </td>
            {columns.map((column) => {
              const result = column.results.get(sample.id)
              return (
                <td key={column.key} className="g-r">
                  {result ? <SampleValue result={result} /> : t('common.none')}
                </td>
              )
            })}
          </tr>
        ))}
      </tbody>
    </Table>
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
    ? t('caseExecution.series.xWithUnit', {
        dimension: xDimension,
        unit: xUnit,
      })
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
    <figure className="tr-derived-figure">
      <figcaption className="body-medium">{title}</figcaption>
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
        <ul className="ui-list tr-derived-without">
          {without.map((sample) => (
            <li key={sample.sample_id}>
              {t('derived.sampleWithout', { repetition: sample.repetition })}{' '}
              <SampleValue result={sample} />
            </li>
          ))}
        </ul>
      )}
    </figure>
  )
}
