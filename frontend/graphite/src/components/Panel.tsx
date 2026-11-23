import type { HTMLAttributes, ReactElement, ReactNode } from 'react';
import { cx } from '../utils/cx';

export interface PanelProps extends Omit<HTMLAttributes<HTMLElement>, 'title'> {
  /** Name the panel after the question it answers. */
  title: ReactNode;
  /** Time window or unit, e.g. "Last 30 days". */
  subtitle?: ReactNode;
  /** Extra header content after the subtitle, e.g. a Badge. */
  meta?: ReactNode;
  /** Right-aligned header actions: small ghost icon Buttons or a Menu. */
  actions?: ReactNode;
  /** Footer: data source and freshness. */
  footer?: ReactNode;
  /** Pad the body with `space-4`. Default true; turn off for tables and full-bleed charts. */
  padded?: boolean;
  children?: ReactNode;
}

/** Container for one chart, table or form. */
export function Panel({ title, subtitle, meta, actions, footer, padded = true, className, children, ...rest }: PanelProps): ReactElement {
  return (
    <section className={cx('g-panel', className)} {...rest}>
      <header className="g-panel-head">
        <h3 className="g-panel-title">{title}</h3>
        {subtitle !== undefined && <span className="g-panel-sub">{subtitle}</span>}
        {meta}
        {actions !== undefined && <div className="g-panel-actions">{actions}</div>}
      </header>
      <div className={cx(padded && 'g-panel-body')} style={{ flex: 1, minHeight: 0 }}>{children}</div>
      {footer !== undefined && <footer className="g-panel-foot">{footer}</footer>}
    </section>
  );
}
