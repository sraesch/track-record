import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'

/** Renders its children into <body>, out of every scrolling container. */
export function Portal({ children }: { children: ReactNode }) {
  return createPortal(children, document.body)
}
