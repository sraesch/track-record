import { cx } from '@graphite/react'
import type { ReactNode } from 'react'
import './attachments.css'

/** Preformatted text in a monospace block; long lines wrap. */
export function CodeBlock({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return <pre className={cx('tr-attachments-code', className)}>{children}</pre>
}

/** A text attachment (a log, CSV, …) as uploaded. */
export function TextView({ text }: { text: string }) {
  return <CodeBlock>{text}</CodeBlock>
}
