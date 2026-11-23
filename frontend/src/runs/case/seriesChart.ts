import type { SampleDetail } from '../../api/client'
import type { ChartColors } from '../../charts/colors'
import type { ChartOption, ChartSeriesOption } from '../../charts/echarts'
import { byRepetition } from './samples'

/**
 * Points per series that a chart asks for. The server keeps the minimum and
 * maximum of each bucket (FR-19), so peaks survive; about one point per
 * pixel of a wide chart.
 */
export const seriesDisplayPoints = 1000

/** One line of a series chart: the series of one sample. */
export type ChartLine = {
  name: string
  /** Position of the sample among all samples; picks its color. */
  colorIndex: number
  warmup: boolean
  x: readonly number[]
  y: readonly number[]
}

type Params = {
  lines: readonly ChartLine[]
  /** Axis names, e.g. "time (s)" and "fps". */
  xName: string
  yName: string
  /** Axis labels: plain numbers, the axis names carry the units. */
  formatTick: (value: number) => string
  /** The tooltip's x (header) and y values, with their units. */
  formatX: (value: number) => string
  formatY: (value: number) => string
  colors: ChartColors
}

/** Dims warmup samples: they are not part of the aggregation (FR-12). */
const warmupOpacity = 0.4

/**
 * The ECharts option for the series of one metric: one line per sample,
 * warmup samples dimmed and dashed. Lines differ by color; past the eighth
 * color they are dotted as well, so no two lines look alike.
 */
export function seriesChartOption({
  lines,
  xName,
  yName,
  formatTick,
  formatX,
  formatY,
  colors,
}: Params): ChartOption {
  const axis = {
    type: 'value' as const,
    scale: true,
    axisLine: { lineStyle: { color: colors.axis } },
    axisTick: { lineStyle: { color: colors.axis } },
    splitLine: { lineStyle: { color: colors.divider } },
    axisLabel: { color: colors.secondaryText, formatter: formatTick },
    nameTextStyle: { color: colors.secondaryText },
  }
  const series = lines.map((line): ChartSeriesOption => {
    const color = colors.lines[line.colorIndex % colors.lines.length]
    const style = line.warmup
      ? 'dashed'
      : line.colorIndex >= colors.lines.length
        ? 'dotted'
        : 'solid'
    const opacity = line.warmup ? warmupOpacity : 1
    return {
      type: 'line',
      name: line.name,
      data: line.x.map((x, i) => [x, line.y[i] ?? Number.NaN]),
      // Only y in the tooltip; x is its header.
      encode: { x: 0, y: 1, tooltip: 1 },
      showSymbol: false,
      color,
      lineStyle: { width: 2, type: style, opacity },
      itemStyle: { opacity },
      emphasis: { focus: 'series' },
      // Measured samples over warmup ones.
      z: line.warmup ? 1 : 2,
    }
  })

  return {
    animation: false,
    textStyle: { fontFamily: colors.fontFamily, color: colors.text },
    grid: { left: 8, right: 24, top: 48, bottom: 8 },
    legend: {
      type: 'scroll',
      top: 0,
      right: 0,
      left: 80,
      textStyle: { color: colors.text },
      inactiveColor: colors.divider,
      pageTextStyle: { color: colors.secondaryText },
    },
    tooltip: {
      trigger: 'axis',
      backgroundColor: colors.surface,
      borderColor: colors.divider,
      textStyle: { color: colors.text },
      valueFormatter: (value) =>
        typeof value === 'number' ? formatY(value) : String(value ?? ''),
    },
    xAxis: {
      ...axis,
      name: xName,
      nameLocation: 'middle',
      nameGap: 28,
      axisPointer: {
        label: { formatter: ({ value }) => formatX(Number(value)) },
      },
    },
    yAxis: {
      ...axis,
      name: yName,
      nameLocation: 'end',
      nameTextStyle: { ...axis.nameTextStyle, align: 'left' },
    },
    series,
  }
}

type SeriesMeta = SampleDetail['series'][number]

/** The series of one metric over one x dimension: one chart. */
export type SeriesGroup = {
  key: string
  /** Name of the metric. */
  metric: string
  unit: string
  xDimension: string
  xUnit: string
  lines: { sample: SampleDetail; series: SeriesMeta; colorIndex: number }[]
}

/**
 * Groups the series of a case execution into charts: one per metric
 * (and x dimension), with one line per sample in repetition order. A sample
 * keeps its color in every chart.
 */
export function groupSeries(samples: readonly SampleDetail[]): SeriesGroup[] {
  const groups = new Map<string, SeriesGroup>()
  byRepetition(samples).forEach((sample, colorIndex) => {
    for (const series of sample.series) {
      const key = [series.metric_id, series.x_dimension, series.x_unit].join(
        '\u0000',
      )
      let group = groups.get(key)
      if (!group) {
        group = {
          key,
          metric: series.metric,
          unit: series.unit,
          xDimension: series.x_dimension,
          xUnit: series.x_unit,
          lines: [],
        }
        groups.set(key, group)
      }
      group.lines.push({ sample, series, colorIndex })
    }
  })
  return [...groups.values()].sort(
    (a, b) =>
      a.metric.localeCompare(b.metric) ||
      a.xDimension.localeCompare(b.xDimension) ||
      a.xUnit.localeCompare(b.xUnit),
  )
}
