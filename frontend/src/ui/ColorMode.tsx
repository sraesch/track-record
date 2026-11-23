import { type ThemePreference, ThemeProvider } from '@graphite/react'
import { type ReactNode, useCallback, useMemo, useState } from 'react'
import { ColorModeContext } from './useColorMode'

// The color mode the user chose in the app bar: follow the system, or light
// or dark. Kept per browser in localStorage, not in the URL (ADR 0011).
// Graphite's ThemeProvider sets `data-theme` on <html>; index.html sets it
// before the app loads, so a reload does not flash the other scheme.

/** The localStorage key; index.html reads it too. */
const colorModeKey = 'track-record.color-mode'

const modes: readonly ThemePreference[] = ['system', 'light', 'dark']

function stored(): ThemePreference {
  try {
    const value = localStorage.getItem(colorModeKey)
    return modes.find((m) => m === value) ?? 'system'
  } catch {
    return 'system'
  }
}

export function ColorModeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState(stored)
  const setMode = useCallback((next: ThemePreference) => {
    setModeState(next)
    try {
      localStorage.setItem(colorModeKey, next)
    } catch {
      // Not stored (private window); it still applies until reload.
    }
  }, [])
  const value = useMemo(() => ({ mode, setMode }), [mode, setMode])
  return (
    <ColorModeContext.Provider value={value}>
      <ThemeProvider theme={mode}>{children}</ThemeProvider>
    </ColorModeContext.Provider>
  )
}
