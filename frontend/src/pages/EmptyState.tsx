import type { ReactNode } from 'react'
import { Surface } from '../ui'
import './pages.css'

type Props = {
  children: ReactNode
  icon?: ReactNode
  /** Dashed border, for something that is not there yet. */
  dashed?: boolean
}

/** Stands in for content that is empty or missing. */
export function EmptyState({ children, icon, dashed }: Props) {
  return (
    <Surface dashed={dashed} className="tr-empty">
      {icon}
      <p>{children}</p>
    </Surface>
  )
}
