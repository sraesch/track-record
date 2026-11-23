import { cx } from '@graphite/react'
import { CircleAlert } from 'lucide-react'
import { forwardRef, type TextareaHTMLAttributes, useId } from 'react'

export type TextAreaProps = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, 'className'> & {
  /** Visible label above the field. */
  label: string
  hideLabel?: boolean
  /** Helper text below the field. Replaced by `error` when present. */
  help?: string
  /** Error message; marks the field invalid. */
  error?: string
  /** Class for the outer wrapper. */
  className?: string
}

/** Multi-line text entry, the counterpart of Graphite's TextField. */
export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(function TextArea(
  { label, hideLabel = false, help, error, className, style, id, rows = 3, disabled, ...rest },
  ref,
) {
  const autoId = useId()
  const fieldId = id ?? autoId
  const messageId = `${fieldId}-msg`
  const message = error ?? help
  return (
    <div className={cx('g-field', className)} style={style}>
      <label htmlFor={fieldId} className={cx('g-label', hideLabel && 'g-visually-hidden')}>
        {label}
      </label>
      <span
        className="g-input ui-textarea"
        {...(error !== undefined ? { 'data-invalid': '' } : {})}
        {...(disabled ? { 'data-disabled': '' } : {})}
      >
        <textarea
          ref={ref}
          id={fieldId}
          rows={rows}
          disabled={disabled}
          aria-invalid={error !== undefined}
          {...(message !== undefined ? { 'aria-describedby': messageId } : {})}
          {...rest}
        />
      </span>
      {error !== undefined ? (
        <span id={messageId} className="g-help g-help-error">
          <CircleAlert size={14} aria-hidden />
          {error}
        </span>
      ) : help !== undefined ? (
        <span id={messageId} className="g-help">
          {help}
        </span>
      ) : null}
    </div>
  )
})
