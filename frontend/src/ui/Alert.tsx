import { cx } from '@graphite/react'
import { CircleAlert, CircleCheck, Info, TriangleAlert } from 'lucide-react'
import type { CSSProperties, ReactNode } from 'react'

export type AlertTone = 'error' | 'warning' | 'info' | 'success'

const icons = { error: CircleAlert, warning: TriangleAlert, info: Info, success: CircleCheck }

type Props = {
  tone: AlertTone
  title?: ReactNode
  /** Buttons on the right, e.g. "Retry" (small ghost buttons). */
  action?: ReactNode
  /** Default `alert` (announced right away); `status` for information that can wait, `none` for none. */
  role?: 'alert' | 'status' | 'none'
  className?: string
  style?: CSSProperties
  children?: ReactNode
}

/** A message with a tone and an icon, e.g. a failed request. */
export function Alert({ tone, title, action, role = 'alert', className, style, children }: Props) {
  const Icon = icons[tone]
  return (
    <div
      {...(role !== 'none' ? { role } : {})}
      className={cx('ui-alert', `ui-alert-${tone}`, className)}
      style={style}
    >
      <Icon size={16} aria-hidden />
      <div className="ui-alert-body">
        {title && <p className="ui-alert-title">{title}</p>}
        {children}
      </div>
      {action && <div className="ui-alert-action">{action}</div>}
    </div>
  )
}
