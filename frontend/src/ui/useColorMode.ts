import type { ThemePreference } from '@graphite/react'
import { createContext, useContext } from 'react'

export type ColorMode = { mode: ThemePreference; setMode: (mode: ThemePreference) => void }

export const ColorModeContext = createContext<ColorMode>({ mode: 'system', setMode: () => {} })

/** The chosen color mode and a setter; `useTheme()` from Graphite gives the resolved one. */
export function useColorMode(): ColorMode {
  return useContext(ColorModeContext)
}
