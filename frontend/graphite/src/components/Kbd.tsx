import type { HTMLAttributes, ReactElement } from 'react';
import { cx } from '../utils/cx';

export interface KbdProps extends HTMLAttributes<HTMLElement> {
  children: string;
}

/** A keyboard shortcut hint, e.g. `⌘K`. */
export function Kbd({ className, children, ...rest }: KbdProps): ReactElement {
  return <kbd className={cx('g-kbd', className)} {...rest}>{children}</kbd>;
}
