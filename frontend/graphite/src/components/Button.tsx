import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { cx } from '../utils/cx';
import { Icon, type IconName } from './Icon';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonCommonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  /** `primary` once per view; `secondary` default; `ghost` for toolbars; `danger` for destructive actions. */
  variant?: ButtonVariant;
  /** `sm` 24px, `md` 28px (default), `lg` 32px. */
  size?: ButtonSize;
}

export interface LabelledButtonProps extends ButtonCommonProps {
  children: ReactNode;
  /** Leading icon. */
  icon?: IconName;
  iconOnly?: false;
}

export interface IconOnlyButtonProps extends ButtonCommonProps {
  icon: IconName;
  iconOnly: true;
  /** Required: icon-only buttons need an accessible name. */
  'aria-label': string;
  children?: never;
}

export type ButtonProps = LabelledButtonProps | IconOnlyButtonProps;

/** Triggers an action. Verb-first, sentence-case labels. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(props, ref) {
  const { variant = 'secondary', size = 'md', icon, iconOnly = false, className, children, type = 'button', ...rest } = props;
  return (
    <button
      ref={ref}
      type={type}
      className={cx('g-btn', `g-btn-${variant}`, size !== 'md' && `g-btn-${size}`, iconOnly && 'g-btn-icon', className)}
      {...rest}
    >
      {icon !== undefined && <Icon name={icon} size={iconOnly && size !== 'sm' ? 'md' : 'sm'} />}
      {children}
    </button>
  );
});
