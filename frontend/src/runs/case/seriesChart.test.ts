import { describe, expect, it } from 'vitest'
import type { ChartColors } from '../../charts/colors'
import type { ChartSeriesOption } from '../../charts/echarts'
import { sample } from '../detail/fixtures'
import { type ChartLine, groupSeries, seriesChartOption } from './seriesChart'

const colors: ChartColors = {
  fontFamily: 'system-ui',
  text: '#000',
  secondaryText: '#555',
  divider: '#ddd',
  surface: '#fff',
  lines: ['#c1', '#c2', '#c3'],
}

const line = (colorIndex: number, overrides: Partial<ChartLine> = {}) => ({
  name: `Repetition ${colorIndex}`,
  colorIndex,
  warmup: false,
  x: [0, 0.5, 1],
  y: [60, 58, 59],
  ...overrides,
})

const option = (lines: ChartLine[]) =>
  seriesChartOption({
    lines,
    xName: 'time (s)',
    yName: 'fps',
    formatTick: String,
    formatX: (value) => `${value} s`,
    formatY: (value) => `${value} fps`,
    colors,
  })

const lines = (lines: ChartLine[]) =>
  [option(lines).series ?? []].flat() as ChartSeriesOption[]

describe('groupSeries', () => {
  it('makes a chart per metric and x dimension with a line per sample', () => {
    const memory = (repetition: number, xDimension = 'time') => ({
      id: `mem-${repetition}`,
      sample_id: `s${repetition}`,
      metric: 'memory',
      metric_id: 11,
      unit: 'MiB',
      x_dimension: xDimension,
      x_unit: 's',
      point_count: 10,
    })
    const samples = [
      sample(2, { series: [...sample(2).series, memory(2)] }),
      sample(0, { series: [...sample(0).series, memory(0)] }),
      sample(1, { series: [...sample(1).series, memory(1, 'uptime')] }),
    ]

    const groups = groupSeries(samples)
    expect(
      groups.map((group) => [
        group.metric,
        group.xDimension,
        group.lines.map((l) => [l.sample.repetition, l.colorIndex]),
      ]),
    ).toEqual([
      ['fps', 'time', [[0, 0], [1, 1], [2, 2]]],
      // A sample keeps its color in every chart.
      ['memory', 'time', [[0, 0], [2, 2]]],
      ['memory', 'uptime', [[1, 1]]],
    ])
    expect(groups[0]).toMatchObject({ unit: 'fps', xDimension: 'time', xUnit: 's' })
  })
})

describe('seriesChartOption', () => {
  it('draws a line per sample from its points', () => {
    const [first] = lines([line(0)])
    expect(first).toMatchObject({
      type: 'line',
      name: 'Repetition 0',
      data: [
        [0, 60],
        [0.5, 58],
        [1, 59],
      ],
      color: '#c1',
      lineStyle: { type: 'solid', opacity: 1 },
    })
  })

  it('dims warmup samples and draws them dashed, below the others', () => {
    const [warmup, measured] = lines([line(0, { warmup: true }), line(1)])
    expect(warmup).toMatchObject({
      color: '#c1',
      lineStyle: { type: 'dashed', opacity: 0.4 },
      z: 1,
    })
    expect(measured).toMatchObject({
      color: '#c2',
      lineStyle: { type: 'solid', opacity: 1 },
      z: 2,
    })
  })

  it('takes the colors in order and dots the lines once they repeat', () => {
    expect(
      lines([line(0), line(2), line(3), line(4)]).map((l) => [
        l.color,
        (l.lineStyle as { type: string }).type,
      ]),
    ).toEqual([
      ['#c1', 'solid'],
      ['#c3', 'solid'],
      ['#c1', 'dotted'],
      ['#c2', 'dotted'],
    ])
  })

  it('names the axes and formats the tooltip with units', () => {
    const result = option([line(0)])
    expect(result.xAxis).toMatchObject({ type: 'value', name: 'time (s)' })
    expect(result.yAxis).toMatchObject({ type: 'value', name: 'fps' })
    const tooltip = result.tooltip as {
      valueFormatter: (value: number) => string
    }
    expect(tooltip.valueFormatter(58)).toBe('58 fps')
  })
})
