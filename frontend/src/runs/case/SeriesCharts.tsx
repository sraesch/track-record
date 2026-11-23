import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { SampleDetail } from '../../api/client'
import { Chart, ChartPlaceholder } from '../../charts/Chart'
import { useChartColors } from '../../charts/colors'
import { formatNumber } from '../../format'
import { ErrorAlert } from '../../pages/ErrorAlert'
import { useSeriesPoints } from '../queries'
import {
  type ChartLine,
  groupSeries,
  type SeriesGroup,
  seriesChartOption,
  seriesDisplayPoints,
} from './seriesChart'
import './case.css'

const chartHeight = 300

/** A chart per series metric of a case execution. */
export function SeriesCharts({
  runId,
  samples,
}: {
  runId: string
  samples: readonly SampleDetail[]
}) {
  const groups = useMemo(() => groupSeries(samples), [samples])
  return (
    <div className="tr-case-charts">
      {groups.map((group) => (
        <SeriesChart
          key={group.key}
          runId={runId}
          group={group}
          title={group.metric}
        />
      ))}
    </div>
  )
}

function SeriesChart({
  runId,
  group,
  title,
}: {
  runId: string
  group: SeriesGroup
  title: string
}) {
  const { t } = useTranslation()
  const colors = useChartColors()
  const seriesIds = useMemo(
    () => group.lines.map((line) => line.series.id),
    [group.lines],
  )
  const { points, isPending, error, retry } = useSeriesPoints(
    runId,
    seriesIds,
    seriesDisplayPoints,
  )

  const xName = group.xUnit
    ? t('caseExecution.series.xWithUnit', {
        dimension: group.xDimension,
        unit: group.xUnit,
      })
    : group.xDimension
  const option = useMemo(() => {
    const withUnit = (unit: string) => (value: number) =>
      unit
        ? t('common.valueWithUnit', { value: formatNumber(value), unit })
        : formatNumber(value)
    const lines = group.lines.flatMap(
      ({ sample, colorIndex }, i): ChartLine[] => {
        const data = points[i]
        if (!data) return []
        return [
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
            x: data.x,
            y: data.y,
          },
        ]
      },
    )
    return seriesChartOption({
      lines,
      xName,
      yName: group.unit,
      formatTick: formatNumber,
      formatX: withUnit(group.xUnit),
      formatY: withUnit(group.unit),
      colors,
    })
  }, [group, points, xName, colors, t])

  return (
    <figure className="tr-case-chart">
      <figcaption className="title-3">{title}</figcaption>
      {error ? (
        <ErrorAlert
          message={t('caseExecution.series.pointsError', {
            message: error.message,
          })}
          onRetry={retry}
        />
      ) : isPending ? (
        <ChartPlaceholder height={chartHeight} />
      ) : (
        <Chart
          option={option}
          height={chartHeight}
          label={t('caseExecution.series.chartLabel', {
            metric: title,
            x: xName,
            count: group.lines.length,
          })}
        />
      )}
    </figure>
  )
}
