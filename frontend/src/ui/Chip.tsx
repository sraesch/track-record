import { cx } from '@graphite/react'
import { type LucideIcon, X } from 'lucide-react'
import { type CSSProperties, forwardRef, type ReactNode } from 'react'
import { Link, type LinkProps } from 'react-router'

export type ChipTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info'

type Props = {
  children: ReactNode
  tone?: ChipTone
  icon?: LucideIcon
  /** Transparent instead of filled. */
  outlined?: boolean
  /** Monospace, for names from uploaded data. */
  mono?: boolean
  /** Makes the chip a button. */
  onClick?: () => void
  /**
   * Adds a remove button, named by `removeLabel`. With `onClick` too, Delete
   * or Backspace on the chip's button removes it as well (say so in its
   * description).
   */
  onRemove?: () => void
  removeLabel?: string
  disabled?: boolean
  /** Name of the chip's button (with `onClick`) or of the chip. */
  'aria-label'?: string
  'aria-describedby'?: string
  title?: string
  className?: string
  style?: CSSProperties
}

/**
 * A short value: a tag, a unit, a status. Clickable (`onClick`) and
 * removable (`onRemove`) when asked; otherwise plain text. For a fixed
 * status, Graphite's `Badge` fits too.
 */
export const Chip = forwardRef<HTMLSpanElement, Props>(function Chip(
  {
    children,
    tone = 'neutral',
    icon: Icon,
    outlined = false,
    mono = false,
    onClick,
    onRemove,
    removeLabel,
    disabled = false,
    className,
    style,
    title,
    ...aria
  },
  ref,
) {
  const content = (
    <>
      {Icon && <Icon size={12} aria-hidden />}
      <span className="ui-chip-label">{children}</span>
    </>
  )
  return (
    <span
      ref={ref}
      className={cx(
        'ui-chip',
        tone !== 'neutral' && `ui-chip-${tone}`,
        outlined && 'ui-chip-outlined',
        mono && 'ui-chip-mono',
        className,
      )}
      style={style}
      title={title}
      {...(disabled ? { 'aria-disabled': true } : {})}
      {...(onClick ? {} : aria)}
    >
      {onClick ? (
        <button
          type="button"
          className="ui-chip-button"
          disabled={disabled}
          onClick={onClick}
          onKeyDown={(event) => {
            if (onRemove && (event.key === 'Delete' || event.key === 'Backspace')) {
              event.preventDefault()
              onRemove()
            }
          }}
          {...aria}
        >
          {content}
        </button>
      ) : (
        content
      )}
      {onRemove && (
        <button
          type="button"
          className="ui-chip-remove"
          // The chip's own button removes it with Delete; one tab stop per chip.
          tabIndex={onClick ? -1 : undefined}
          aria-label={removeLabel}
          disabled={disabled}
          onClick={onRemove}
        >
          <X size={12} aria-hidden />
        </button>
      )}
    </span>
  )
})

type ChipLinkProps = LinkProps & {
  children: ReactNode
  icon?: LucideIcon
  mono?: boolean
}

/** A chip that is a link inside the app, e.g. a tag that opens what it is on. */
export function ChipLink({ children, icon: Icon, mono = false, className, ...rest }: ChipLinkProps) {
  return (
    <Link className={cx('ui-chip', 'ui-chip-link', mono && 'ui-chip-mono', className)} {...rest}>
      {Icon && <Icon size={12} aria-hidden />}
      <span className="ui-chip-label">{children}</span>
    </Link>
  )
}
