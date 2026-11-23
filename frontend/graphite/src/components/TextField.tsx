import { forwardRef, useId, type InputHTMLAttributes, type ReactNode } from 'react';
import { cx } from '../utils/cx';
import { Icon, type IconName } from './Icon';

export interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'size' | 'prefix'> {
  /** Visible label above the input. Required — placeholders are not labels. */
  label: string;
  /** Visually hide the label (it stays available to assistive tech). */
  hideLabel?: boolean;
  /** Helper text below the input. Replaced by `error` when present. */
  help?: string;
  /** Error message; marks the field invalid. Say how to fix it. */
  error?: string;
  /** Leading icon, e.g. `search`. */
  icon?: IconName;
  /** Trailing content inside the field: a unit, a `Kbd` hint. */
  trailing?: ReactNode;
  /** Align the value right and use tabular figures. */
  numeric?: boolean;
  /** Class for the outer wrapper. */
  className?: string;
}

/** Single-line text entry with label, help and error. */
export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, hideLabel = false, help, error, icon, trailing, numeric = false, className, disabled, id, style, ...rest },
  ref,
) {
  const autoId = useId();
  const inputId = id ?? autoId;
  const messageId = `${inputId}-msg`;
  const message = error ?? help;
  return (
    <div className={cx('g-field', className)} style={style}>
      <label htmlFor={inputId} className={cx('g-label', hideLabel && 'g-visually-hidden')}>{label}</label>
      <span
        className="g-input"
        {...(error !== undefined ? { 'data-invalid': '' } : {})}
        {...(disabled === true ? { 'data-disabled': '' } : {})}
      >
        {icon !== undefined && <Icon name={icon} size="sm" />}
        <input
          ref={ref}
          id={inputId}
          disabled={disabled}
          aria-invalid={error !== undefined}
          className={cx(numeric && 'g-num')}
          style={numeric ? { textAlign: 'right' } : undefined}
          {...(message !== undefined ? { 'aria-describedby': messageId } : {})}
          {...rest}
        />
        {trailing}
      </span>
      {error !== undefined ? (
        <span id={messageId} className="g-help g-help-error"><Icon name="alert" size="sm" />{error}</span>
      ) : help !== undefined ? (
        <span id={messageId} className="g-help">{help}</span>
      ) : null}
    </div>
  );
});
