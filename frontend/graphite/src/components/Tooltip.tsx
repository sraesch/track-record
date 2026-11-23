import { cloneElement, useEffect, useId, useRef, useState, type ReactElement, type ReactNode } from 'react';
import { cx } from '../utils/cx';

type DescribableProps = {
  'aria-describedby'?: string | undefined;
};

export interface TooltipProps {
  /** Short, supplementary text. Never essential or interactive content. */
  content: ReactNode;
  /** A single focusable element. */
  children: ReactElement<DescribableProps>;
  placement?: 'top' | 'bottom';
  /** Delay before showing, ms. Default 400. */
  delay?: number;
}

/** Shows on hover and keyboard focus after a delay; Esc hides it. */
export function Tooltip({ content, children, placement = 'top', delay = 400 }: TooltipProps): ReactElement {
  const [open, setOpen] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const id = useId();
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const show = (): void => { window.clearTimeout(timer.current); timer.current = window.setTimeout(() => setOpen(true), delay); };
  const hide = (): void => { window.clearTimeout(timer.current); setOpen(false); };
  return (
    <span
      className="g-anchor"
      onPointerEnter={show}
      onPointerLeave={hide}
      onFocus={show}
      onBlur={hide}
      onKeyDown={(e) => { if (e.key === 'Escape') hide(); }}
    >
      {cloneElement(children, { 'aria-describedby': open ? id : children.props['aria-describedby'] })}
      {open && (
        <span id={id} role="tooltip" className={cx('g-tooltip g-tip', placement === 'bottom' && 'g-tip-bottom')}>
          {content}
        </span>
      )}
    </span>
  );
}

export interface TooltipRow {
  label: string;
  /** Formatted value. */
  value: string;
  /** Swatch colour, e.g. `chartColor(1)`. */
  color?: string;
}

export interface TooltipContentProps {
  title?: ReactNode;
  rows: readonly TooltipRow[];
}

/** Structured tooltip body for charts: a title (the x value) and one row per series. */
export function TooltipContent({ title, rows }: TooltipContentProps): ReactElement {
  return (
    <>
      {title !== undefined && <span className="g-tooltip-title">{title}</span>}
      {rows.map((r) => (
        <span key={r.label} className="g-tooltip-row">
          {r.color !== undefined && <span className="g-swatch" style={{ background: r.color }} />}
          {r.label}
          <b>{r.value}</b>
        </span>
      ))}
    </>
  );
}
