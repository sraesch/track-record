import { ChevronDown } from 'lucide-react'
import { type ReactNode, useId, useState } from 'react'

type Props = {
  /** The heading that opens and closes the content. */
  summary: ReactNode
  children: ReactNode
  defaultOpen?: boolean
  /** Heading level of the summary. */
  level?: 2 | 3 | 4
  /** Mount the content only while open. Default true. */
  lazy?: boolean
}

/** A section that opens and closes (accordion item, WAI-ARIA disclosure). */
export function Disclosure({ summary, children, defaultOpen = false, level = 3, lazy = true }: Props) {
  const [open, setOpen] = useState(defaultOpen)
  const id = useId()
  const Heading = `h${level}` as const
  return (
    <div className="ui-disclosure">
      <Heading className="ui-disclosure-head">
        <button
          type="button"
          id={`${id}-button`}
          className="ui-disclosure-button"
          aria-expanded={open}
          aria-controls={`${id}-panel`}
          onClick={() => setOpen((o) => !o)}
        >
          {summary}
          <ChevronDown size={16} aria-hidden />
        </button>
      </Heading>
      <section
        id={`${id}-panel`}
        aria-labelledby={`${id}-button`}
        className="ui-disclosure-panel"
        hidden={!open}
      >
        {(open || !lazy) && children}
      </section>
    </div>
  )
}
