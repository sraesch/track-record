import { Fragment, type ReactNode } from 'react'
import './pages.css'

export type Fact = {
  label: string
  value: ReactNode
  /** The label in monospace, e.g. a key from uploaded data. */
  code?: boolean
}

/**
 * Label/value pairs as a description list. Where there is room, the pairs
 * are laid out in two columns.
 */
export function Facts({ items }: { items: readonly Fact[] }) {
  return (
    <div className="tr-facts">
      <dl>
        {items.map(({ label, value, code }) => (
          <Fragment key={label}>
            <dt className={code ? 'tr-facts-code' : undefined}>{label}</dt>
            <dd>{value}</dd>
          </Fragment>
        ))}
      </dl>
    </div>
  )
}
