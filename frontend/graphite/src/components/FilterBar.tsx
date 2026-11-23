import type { HTMLAttributes, MouseEvent, ReactElement, ReactNode } from 'react';
import { cx } from '../utils/cx';
import { Icon, type IconName } from './Icon';

export interface FilterBarProps extends HTMLAttributes<HTMLDivElement> {
  /** FilterChips, a FilterBarSeparator after the time range, then an "Add filter" Button. */
  children: ReactNode;
  /** Right-aligned content, e.g. a "Clear all" ghost Button. */
  end?: ReactNode;
  /** Accessible name of the toolbar. Default "Filters". */
  label?: string;
}

/** One row holding every filter for the view. */
export function FilterBar({ children, end, label = 'Filters', className, ...rest }: FilterBarProps): ReactElement {
  return (
    <div role="toolbar" aria-label={label} className={cx('g-filterbar', className)} {...rest}>
      {children}
      {end !== undefined && <div style={{ marginLeft: 'auto', display: 'flex', gap: 'var(--space-2)' }}>{end}</div>}
    </div>
  );
}

/** Vertical divider between the time range and dimension filters. */
export function FilterBarSeparator(): ReactElement {
  return <span className="g-filterbar-sep" aria-hidden="true" />;
}

export interface FilterChipProps {
  /** Field name, e.g. "Region". Omit for the time-range chip. */
  field?: string;
  /** Operator, e.g. "is", "is not", "≥". Default "is" when `field` is set. */
  operator?: string;
  /** Value summary, e.g. "EU-West, EU-South" or "3 regions". */
  value: string;
  icon?: IconName;
  /** Applied filters are active (accent tint). Default true. */
  active?: boolean;
  /** Opens the editor popover for this filter. */
  onClick?: () => void;
  /** Shows a remove button. */
  onRemove?: () => void;
  /** Show a dropdown chevron (time range, single-select filters). */
  chevron?: boolean;
  /** Accessible name of the remove button. Default "Remove filter <field>". */
  removeLabel?: string;
}

/** A filter that reads as a sentence: field, operator, value. */
export function FilterChip({ field, operator = 'is', value, icon, active = true, onClick, onRemove, chevron = false, removeLabel }: FilterChipProps): ReactElement {
  const remove = (e: MouseEvent): void => { e.stopPropagation(); onRemove?.(); };
  return (
    <span className="g-chip" {...(active ? { 'data-active': '' } : {})}>
      <button type="button" className="g-chip-main" onClick={onClick}>
        {icon !== undefined && <Icon name={icon} size="sm" />}
        {field !== undefined && <span>{field} {operator}</span>}
        <b>{value}</b>
        {chevron && <Icon name="chevron-down" size="sm" />}
      </button>
      {onRemove !== undefined && (
        <button type="button" className="g-chip-x" aria-label={removeLabel ?? `Remove filter ${field ?? value}`} onClick={remove}>
          <Icon name="x" size="sm" />
        </button>
      )}
    </span>
  );
}
