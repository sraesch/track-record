// ECharts with only what the app uses (ADR 0015), so the rest of the
// library stays out of the bundle. Imported only by the lazily loaded
// chart (EChart.tsx); other modules import types from here, nothing else.
import { LineChart, type LineSeriesOption } from 'echarts/charts'
import {
  GridComponent,
  type GridComponentOption,
  LegendComponent,
  type LegendComponentOption,
  TooltipComponent,
  type TooltipComponentOption,
} from 'echarts/components'
import { type ComposeOption, init, use as register } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'

register([
  LineChart,
  GridComponent,
  LegendComponent,
  TooltipComponent,
  CanvasRenderer,
])

export { init }

/** The options the app's charts can use. */
export type ChartOption = ComposeOption<
  | LineSeriesOption
  | GridComponentOption
  | LegendComponentOption
  | TooltipComponentOption
>

export type ChartSeriesOption = LineSeriesOption
