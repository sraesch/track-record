import {
  type CssVarsTheme,
  useColorScheme,
  useTheme,
} from '@mui/material/styles'
import { useMemo } from 'react'

// Chart colors of the active color scheme (ADR 0015). The MUI theme keeps
// its colors in CSS variables, which a canvas cannot read, so they are
// resolved here to concrete values.

/**
 * Categorical line colors in a fixed order, one set per color scheme: the
 * same hues, stepped for the light and the dark surface. The order keeps
 * neighbouring colors apart for color-vision deficiencies; don't reorder.
 */
const lineColors = {
  light: ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#008300', '#6250d6', '#e34948'],
  dark: ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181', '#0ca30c', '#9085e9', '#e66767'],
} as const

export type ChartColors = {
  /** The app's font, for axis labels, legend and tooltips. */
  fontFamily: string
  text: string
  secondaryText: string
  /** Axis and grid lines. */
  divider: string
  /** Behind tooltips. */
  surface: string
  lines: readonly string[]
}

export function useChartColors(): ChartColors {
  const theme = useTheme()
  const { mode, systemMode } = useColorScheme()
  const scheme =
    (mode === 'system' ? systemMode : mode) === 'dark' ? 'dark' : 'light'
  // The theme has color schemes (src/theme.ts); `Theme` does not say so.
  const schemes = (theme as { colorSchemes?: CssVarsTheme['colorSchemes'] })
    .colorSchemes
  const palette = schemes?.[scheme]?.palette ?? theme.palette
  return useMemo(
    () => ({
      fontFamily: theme.typography.fontFamily ?? 'sans-serif',
      text: palette.text.primary,
      secondaryText: palette.text.secondary,
      divider: palette.divider,
      surface: palette.background.paper,
      lines: lineColors[scheme],
    }),
    [palette, scheme, theme.typography.fontFamily],
  )
}
