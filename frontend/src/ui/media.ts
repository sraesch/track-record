import { useSyncExternalStore } from 'react'

/** Breakpoints: below `md` (900px) pages stack their panes, below `sm` (600px) dialogs fill the screen. */
export const breakpoints = { sm: 600, md: 900 } as const

/** Whether a media query matches, following changes (e.g. `(min-width:900px)`). */
export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const list = window.matchMedia(query)
      list.addEventListener('change', onChange)
      return () => list.removeEventListener('change', onChange)
    },
    () => window.matchMedia(query).matches,
    () => false,
  )
}
