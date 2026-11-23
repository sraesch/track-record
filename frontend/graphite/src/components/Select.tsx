import { useCallback, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactElement } from 'react';
import { cx } from '../utils/cx';
import { useControllableState, useDismiss } from '../utils/hooks';
import { Icon } from './Icon';

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SelectProps<T extends string> {
  /** Visible label above the trigger. */
  label: string;
  hideLabel?: boolean;
  /** Up to ~15 options, in the order people expect. */
  options: readonly SelectOption<T>[];
  value?: T;
  defaultValue?: T;
  onChange?: (value: T) => void;
  /** Shown when nothing is selected. */
  placeholder?: string;
  disabled?: boolean;
  help?: string;
  className?: string;
}

/** Pick one value from a short list. Keyboard: arrows, Home/End, Enter, Esc, type-ahead. */
export function Select<T extends string>({
  label,
  hideLabel = false,
  options,
  value,
  defaultValue,
  onChange,
  placeholder = 'Select…',
  disabled = false,
  help,
  className,
}: SelectProps<T>): ReactElement {
  const id = useId();
  const [selected, setSelected] = useControllableState<T | undefined>(value, defaultValue, onChange as ((v: T | undefined) => void) | undefined);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const wrapRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const refs = useMemo(() => [wrapRef], []);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(refs, open, close);

  const selectedOption = options.find((o) => o.value === selected);
  const enabled = (i: number): boolean => options[i] !== undefined && options[i]?.disabled !== true;

  const move = (from: number, step: 1 | -1): number => {
    for (let i = 1; i <= options.length; i++) {
      const next = (from + step * i + options.length) % options.length;
      if (enabled(next)) return next;
    }
    return from;
  };

  const openList = (): void => {
    const idx = options.findIndex((o) => o.value === selected);
    setActive(idx >= 0 ? idx : move(-1, 1));
    setOpen(true);
  };

  const commit = (i: number): void => {
    const opt = options[i];
    if (opt === undefined || opt.disabled === true) return;
    setSelected(opt.value);
    setOpen(false);
    triggerRef.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>): void => {
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        openList();
      }
      return;
    }
    switch (e.key) {
      case 'ArrowDown': e.preventDefault(); setActive((a) => move(a, 1)); break;
      case 'ArrowUp': e.preventDefault(); setActive((a) => move(a, -1)); break;
      case 'Home': e.preventDefault(); setActive(move(-1, 1)); break;
      case 'End': e.preventDefault(); setActive(move(options.length, -1)); break;
      case 'Enter': case ' ': e.preventDefault(); commit(active); break;
      case 'Escape': case 'Tab': setOpen(false); break;
      default:
        if (e.key.length === 1) {
          const ch = e.key.toLowerCase();
          const hit = options.findIndex((o, i) => i > active && o.disabled !== true && o.label.toLowerCase().startsWith(ch));
          const wrap = hit >= 0 ? hit : options.findIndex((o) => o.disabled !== true && o.label.toLowerCase().startsWith(ch));
          if (wrap >= 0) setActive(wrap);
        }
    }
  };

  const listId = `${id}-list`;
  return (
    <div className={cx('g-field', className)} ref={wrapRef}>
      <span id={`${id}-label`} className={cx('g-label', hideLabel && 'g-visually-hidden')}>{label}</span>
      <div className="g-anchor">
        <button
          ref={triggerRef}
          type="button"
          className="g-input g-select"
          disabled={disabled}
          role="combobox"
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-controls={listId}
          aria-labelledby={`${id}-label`}
          {...(open && active >= 0 ? { 'aria-activedescendant': `${id}-opt-${active}` } : {})}
          {...(disabled ? { 'data-disabled': '' } : {})}
          onClick={() => (open ? setOpen(false) : openList())}
          onKeyDown={onKeyDown}
        >
          <span id={`${id}-value`} style={selectedOption === undefined ? { color: 'var(--text-muted)' } : undefined}>
            {selectedOption?.label ?? placeholder}
          </span>
          <Icon name="chevron-down" size="sm" />
        </button>
        {open && (
          <ul id={listId} role="listbox" aria-labelledby={`${id}-label`} className="g-menu g-pop g-listbox">
            {options.map((opt, i) => (
              <li
                key={opt.value}
                id={`${id}-opt-${i}`}
                role="option"
                aria-selected={opt.value === selected}
                aria-disabled={opt.disabled === true}
                className="g-menu-item"
                {...(i === active ? { 'data-active': '' } : {})}
                {...(opt.value === selected ? { 'aria-checked': true } : {})}
                onPointerEnter={() => enabled(i) && setActive(i)}
                onPointerDown={(e) => e.preventDefault()}
                onClick={() => commit(i)}
              >
                {opt.label}
                {opt.value === selected && <Icon name="check" size="sm" style={{ marginLeft: 'auto' }} />}
              </li>
            ))}
          </ul>
        )}
      </div>
      {help !== undefined && <span className="g-help">{help}</span>}
    </div>
  );
}
