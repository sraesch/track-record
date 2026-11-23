import { useRef, type KeyboardEvent, type ReactElement } from 'react';
import { cx } from '../utils/cx';
import { useControllableState } from '../utils/hooks';
import { Icon, type IconName } from './Icon';

export interface TabItem<T extends string> {
  value: T;
  label: string;
  /** Optional item count shown as a pill. */
  count?: number;
  disabled?: boolean;
}

export interface TabsProps<T extends string> {
  items: readonly TabItem<T>[];
  value?: T;
  defaultValue?: T;
  onChange?: (value: T) => void;
  /** Accessible name of the tab list. */
  'aria-label': string;
  /** Id prefix used for `aria-controls`; the consumer renders panels with id `${idPrefix}-panel-${value}`. */
  idPrefix?: string;
  className?: string;
}

function rovingKeys<T>(e: KeyboardEvent, items: readonly { value: T; disabled?: boolean }[], current: T, select: (v: T) => void, buttons: (HTMLButtonElement | null)[]): void {
  const enabled = items.filter((i) => i.disabled !== true);
  const idx = enabled.findIndex((i) => i.value === current);
  let next: number | undefined;
  if (e.key === 'ArrowRight') next = (idx + 1) % enabled.length;
  else if (e.key === 'ArrowLeft') next = (idx - 1 + enabled.length) % enabled.length;
  else if (e.key === 'Home') next = 0;
  else if (e.key === 'End') next = enabled.length - 1;
  if (next === undefined) return;
  e.preventDefault();
  const item = enabled[next];
  if (item === undefined) return;
  select(item.value);
  buttons[items.indexOf(item)]?.focus();
}

/** Switch between views of the same object. Renders the tab list only; the consumer renders the panel. */
export function Tabs<T extends string>({ items, value, defaultValue, onChange, idPrefix, className, ...aria }: TabsProps<T>): ReactElement {
  const first = items[0]?.value;
  const [current, setCurrent] = useControllableState<T | undefined>(value, defaultValue ?? first, onChange as ((v: T | undefined) => void) | undefined);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  return (
    <div role="tablist" aria-label={aria['aria-label']} className={cx('g-tabs', className)}>
      {items.map((item, i) => {
        const selected = item.value === current;
        return (
          <button
            key={item.value}
            ref={(el) => { buttons.current[i] = el; }}
            type="button"
            role="tab"
            className="g-tab"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            disabled={item.disabled}
            {...(idPrefix !== undefined ? { id: `${idPrefix}-tab-${item.value}`, 'aria-controls': `${idPrefix}-panel-${item.value}` } : {})}
            onClick={() => setCurrent(item.value)}
            onKeyDown={(e) => current !== undefined && rovingKeys(e, items, current, setCurrent, buttons.current)}
          >
            {item.label}
            {item.count !== undefined && <span className="g-count">{item.count}</span>}
          </button>
        );
      })}
    </div>
  );
}

export type SegmentItem<T extends string> =
  | { value: T; label: string; icon?: IconName }
  | { value: T; icon: IconName; 'aria-label': string; label?: never };

export interface SegmentedControlProps<T extends string> {
  items: readonly SegmentItem<T>[];
  value?: T;
  defaultValue?: T;
  onChange?: (value: T) => void;
  'aria-label': string;
  className?: string;
}

/** Switch a mode of the current view: granularity, chart vs table, units. */
export function SegmentedControl<T extends string>({ items, value, defaultValue, onChange, className, ...aria }: SegmentedControlProps<T>): ReactElement {
  const first = items[0]?.value;
  const [current, setCurrent] = useControllableState<T | undefined>(value, defaultValue ?? first, onChange as ((v: T | undefined) => void) | undefined);
  return (
    <div role="group" aria-label={aria['aria-label']} className={cx('g-seg', className)}>
      {items.map((item) => (
        <button
          key={item.value}
          type="button"
          aria-pressed={item.value === current}
          {...('aria-label' in item && item['aria-label'] !== undefined ? { 'aria-label': item['aria-label'] } : {})}
          onClick={() => setCurrent(item.value)}
        >
          {item.icon !== undefined && <Icon name={item.icon} size="sm" />}
          {item.label}
        </button>
      ))}
    </div>
  );
}
