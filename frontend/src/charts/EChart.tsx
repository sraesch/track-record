import { useEffect, useRef } from 'react'
import { type ChartOption, init } from './echarts'

export type EChartProps = {
  option: ChartOption
  /** Height in pixels; the chart takes the width of its container. */
  height: number
}

/**
 * An ECharts chart on a canvas (ADR 0015): created on mount, updated with
 * every new option, resized with its container, disposed on unmount.
 * Loaded lazily through `Chart`; use that instead.
 */
export function EChart({ option, height }: EChartProps) {
  const container = useRef<HTMLDivElement>(null)
  const chart = useRef<ReturnType<typeof init>>(null)

  useEffect(() => {
    const element = container.current
    if (!element) return
    const instance = init(element, null, { renderer: 'canvas' })
    chart.current = instance
    const observer = new ResizeObserver(() => instance.resize())
    observer.observe(element)
    return () => {
      observer.disconnect()
      instance.dispose()
      chart.current = null
    }
  }, [])

  // Runs after the effect above, also when both run on mount.
  useEffect(() => {
    // Replace, don't merge: lines that are gone must disappear.
    chart.current?.setOption(option, { notMerge: true })
  }, [option])

  return (
    // A canvas means nothing to a screen reader; `Chart` describes it.
    <div ref={container} aria-hidden style={{ width: '100%', height }} />
  )
}
