import type { ReactNode } from 'react'
import { useId } from 'react'
import './pages.css'

type Props = {
  title: string
  children: ReactNode
  /** Shown to the right of the title, e.g. a count or actions. */
  actions?: ReactNode
  /** No padding around the content, for a table that fills the section. */
  flush?: boolean
}

/** A titled block of a page (Graphite's panel), e.g. one part of a run's detail. */
export function Section({ title, children, actions, flush }: Props) {
  const titleId = useId()
  return (
    <section aria-labelledby={titleId} className="g-panel tr-section">
      <header className="g-panel-head">
        <h2 id={titleId} className="g-panel-title">
          {title}
        </h2>
        {actions && <div className="g-panel-actions">{actions}</div>}
      </header>
      <div className={flush ? undefined : 'tr-section-body'}>{children}</div>
    </section>
  )
}
