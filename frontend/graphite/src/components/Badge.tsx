import type { HTMLAttributes, ReactElement, ReactNode } from 'react';
import { cx } from '../utils/cx';
import { Icon, type IconName } from './Icon';

export type BadgeTone = 'neutral' | 'success' | 'warning' | 'danger' | 'info' | 'accent';

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  /** Status tones always need a dot or icon too, so meaning never rests on colour alone. */
  tone?: BadgeTone;
  /** Leading status dot. */
  dot?: boolean;
  /** Leading 14px icon (takes precedence over `dot`). */
  icon?: IconName;
  children: ReactNode;
}

/** One or two words of status or attribute. Not interactive. */
export function Badge({ tone = 'neutral', dot = false, icon, className, children, ...rest }: BadgeProps): ReactElement {
  return (
    <span className={cx('g-badge', tone !== 'neutral' && `g-badge-${tone}`, className)} {...rest}>
      {icon !== undefined ? <Icon name={icon} size="sm" /> : dot && <span className="g-dot" aria-hidden="true" />}
      {children}
    </span>
  );
}
