import { type ThemeName, useTheme } from '@graphite/react'
import { useMemo } from 'react'

// Chart colors of the active color scheme (ADR 0015, ADR 0019): Graphite's
// tokens, which a canvas cannot read as CSS variables, resolved here to
// concrete values. The fallbacks are the token values, for when the style
// sheet is not there (tests).

const fallback = {
  light: {
    font: '"Geist", "Segoe UI", system-ui, sans-serif',
    text: '#14171c',
    secondaryText: '#5f6876',
    grid: '#eceef1',
    axis: '#b9bfc8',
    surface: '#f1f3f5',
    lines: [
      '#2a78d6',
      '#eb6834',
      '#1baf7a',
      '#eda100',
      '#e87ba4',
      '#008300',
      '#6250d6',
      '#e34948',
    ],
  },
  dark: {
    font: '"Geist", "Segoe UI", system-ui, sans-serif',
    text: '#e8ebf0',
    secondaryText: '#8a94a3',
    grid: '#232932',
    axis: '#3a424e',
    surface: '#1d222a',
    lines: [
      '#3987e5',
      '#d95926',
      '#199e70',
      '#c98500',
      '#d55181',
      '#0ca30c',
      '#9085e9',
      '#e66767',
    ],
  },
} as const

export type ChartColors = {
  /** The app's font, for axis labels, legend and tooltips. */
  fontFamily: string
  text: string
  secondaryText: string
  /** Grid lines. */
  divider: string
  /** Axis lines. */
  axis: string
  /** Behind tooltips. */
  surface: string
  /**
   * Categorical line colors (Graphite's chart slots) in a fixed order. The
   * order keeps neighbouring colors apart for color-vision deficiencies;
   * don't reorder.
   */
  lines: readonly string[]
}

function resolve(theme: ThemeName): ChartColors {
  const base = fallback[theme]
  // The tokens are defined per [data-theme]; a probe with the theme reads
  // them, whatever <html> says at this moment (Graphite's ThemeProvider
  // updates it after the charts render).
  const probe = document.createElement('div')
  probe.setAttribute('data-theme', theme)
  probe.hidden = true
  document.body.append(probe)
  const style = getComputedStyle(probe)
  const token = (name: string, otherwise: string) =>
    style.getPropertyValue(name).trim() || otherwise
  const colors = {
    fontFamily: token('--font-sans', base.font),
    text: token('--text-primary', base.text),
    secondaryText: token('--text-muted', base.secondaryText),
    divider: token('--chart-grid', base.grid),
    axis: token('--chart-axis', base.axis),
    surface: token('--bg-300', base.surface),
    lines: base.lines.map((line, i) => token(`--chart-${i + 1}`, line)),
  }
  probe.remove()
  return colors
}

export function useChartColors(): ChartColors {
  const { resolved } = useTheme()
  return useMemo(() => resolve(resolved), [resolved])
}
