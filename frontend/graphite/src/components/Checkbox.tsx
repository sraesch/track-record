import { forwardRef, useEffect, useRef, type InputHTMLAttributes, type ReactElement, type ReactNode } from 'react';
import { cx } from '../utils/cx';
import { useControllableState, useMergedRef } from '../utils/hooks';

type NativeInput = Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'children' | 'onChange' | 'checked' | 'defaultChecked'>;

export interface CheckboxProps extends NativeInput {
  /** Clickable label to the right of the box. Omit only inside table cells (then pass `aria-label`). */
  label?: ReactNode;
  checked?: boolean;
  defaultChecked?: boolean;
  /** Partial selection (e.g. "select all" with some rows selected). */
  indeterminate?: boolean;
  onCheckedChange?: (checked: boolean) => void;
}

/** Independent on/off option, or multi-select in lists and tables. */
export const Checkbox = forwardRef<HTMLInputElement, CheckboxProps>(function Checkbox(
  { label, indeterminate = false, onCheckedChange, className, checked, defaultChecked, ...rest },
  ref,
) {
  const inner = useRef<HTMLInputElement>(null);
  const merged = useMergedRef(ref, inner);
  useEffect(() => {
    if (inner.current !== null) inner.current.indeterminate = indeterminate;
  }, [indeterminate]);
  return (
    <label className={cx('g-check', className)}>
      <input
        ref={merged}
        type="checkbox"
        {...(checked !== undefined ? { checked } : {})}
        {...(defaultChecked !== undefined ? { defaultChecked } : {})}
        onChange={(e) => onCheckedChange?.(e.currentTarget.checked)}
        {...rest}
      />
      {label}
    </label>
  );
});

export interface SwitchProps extends NativeInput {
  label: ReactNode;
  checked?: boolean;
  defaultChecked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
}

/** A setting that applies immediately. If it needs Save, use a Checkbox. */
export const Switch = forwardRef<HTMLInputElement, SwitchProps>(function Switch(
  { label, onCheckedChange, className, checked, defaultChecked, ...rest },
  ref,
) {
  return (
    <label className={cx('g-check g-switch', className)}>
      <input
        ref={ref}
        type="checkbox"
        role="switch"
        {...(checked !== undefined ? { checked } : {})}
        {...(defaultChecked !== undefined ? { defaultChecked } : {})}
        onChange={(e) => onCheckedChange?.(e.currentTarget.checked)}
        {...rest}
      />
      {label}
    </label>
  );
});

export interface RadioOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface RadioGroupProps<T extends string> {
  /** Accessible name of the group. */
  label: string;
  /** Show `label` above the options. Default false. */
  showLabel?: boolean;
  options: readonly RadioOption<T>[];
  value?: T;
  defaultValue?: T;
  onChange?: (value: T) => void;
  /** Native `name`; generated when omitted. */
  name?: string;
  direction?: 'vertical' | 'horizontal';
  className?: string;
}

let radioGroupCount = 0;

/** One of 2–5 mutually exclusive options shown together. */
export function RadioGroup<T extends string>({
  label,
  showLabel = false,
  options,
  value,
  defaultValue,
  onChange,
  name,
  direction = 'vertical',
  className,
}: RadioGroupProps<T>): ReactElement {
  const first = options[0];
  const [current, setCurrent] = useControllableState<T | undefined>(value, defaultValue ?? first?.value, onChange as ((v: T | undefined) => void) | undefined);
  const groupName = useRef(name ?? `g-radio-${++radioGroupCount}`).current;
  return (
    <div role="radiogroup" aria-label={label} className={cx('g-stack', className)} style={direction === 'horizontal' ? { flexDirection: 'row', gap: 'var(--space-4)' } : { gap: 'var(--space-2)' }}>
      {showLabel && <span className="g-label">{label}</span>}
      {options.map((opt) => (
        <label key={opt.value} className="g-check">
          <input
            type="radio"
            name={groupName}
            value={opt.value}
            checked={current === opt.value}
            disabled={opt.disabled}
            onChange={() => setCurrent(opt.value)}
          />
          {opt.label}
        </label>
      ))}
    </div>
  );
}
