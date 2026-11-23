// WAI-ARIA combobox and listbox pattern: the options are <li role="option">
// in a <ul role="listbox">; the focus stays on the field, which handles the
// keys (aria-activedescendant), so the options only take clicks.
/* oxlint-disable jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role, jsx-a11y/click-events-have-key-events */
import { cx } from '@graphite/react'
import { Check, ChevronDown } from 'lucide-react'
import {
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useId,
  useRef,
  useState,
} from 'react'
import { Portal } from './Popover'
import { useDismiss, usePopoverStyle } from './usePopover'

export type SelectOption<T extends string> = {
  value: T
  label: ReactNode
  /** Text for type-ahead and the closed field, when `label` is not a string. */
  text?: string
  disabled?: boolean
}

type Common = {
  /** Visible label above the field. */
  label: string
  hideLabel?: boolean
  disabled?: boolean
  help?: string
  className?: string
  style?: CSSProperties
}

export type SelectProps<T extends string> = Common & {
  options: readonly SelectOption<T>[]
  value: T
  onChange: (value: T) => void
}

export type MultiSelectProps<T extends string> = Common & {
  options: readonly SelectOption<T>[]
  value: readonly T[]
  onChange: (value: T[]) => void
  /** Shown when nothing is selected. */
  placeholder?: string
}

const textOf = (option: SelectOption<string>) =>
  option.text ?? (typeof option.label === 'string' ? option.label : String(option.value))

/**
 * Picks one value from a short list (Graphite's select, with its list in a
 * popover so dialogs and tables do not clip it). For "any", add an option
 * with an empty value. Keyboard: arrows, Home/End, Enter, Escape, type-ahead.
 */
export function Select<T extends string>({ options, value, onChange, ...common }: SelectProps<T>) {
  const selected = options.find((o) => o.value === value)
  return (
    <ListSelect
      {...common}
      options={options}
      isSelected={(v) => v === value}
      display={selected ? selected.label : ''}
      onPick={(v) => {
        onChange(v)
        return true
      }}
    />
  )
}

/** Picks any number of values from a short list; the list stays open. */
export function MultiSelect<T extends string>({ options, value, onChange, placeholder, ...common }: MultiSelectProps<T>) {
  const shown = options.filter((o) => value.includes(o.value)).map(textOf)
  return (
    <ListSelect
      {...common}
      multiple
      options={options}
      isSelected={(v) => value.includes(v)}
      display={shown.length > 0 ? shown.join(', ') : <span className="ui-select-placeholder">{placeholder}</span>}
      onPick={(v) => {
        onChange(value.includes(v) ? value.filter((x) => x !== v) : [...value, v])
        return false
      }}
    />
  )
}

type ListSelectProps<T extends string> = Common & {
  options: readonly SelectOption<T>[]
  isSelected: (value: T) => boolean
  display: ReactNode
  /** Returns whether to close the list. */
  onPick: (value: T) => boolean
  multiple?: boolean
}

function ListSelect<T extends string>({
  label,
  hideLabel,
  disabled,
  help,
  className,
  style,
  options,
  isSelected,
  display,
  onPick,
  multiple = false,
}: ListSelectProps<T>) {
  const id = useId()
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)
  const trigger = useRef<HTMLButtonElement>(null)
  const list = useRef<HTMLUListElement>(null)
  const popover = usePopoverStyle(trigger, open, { matchWidth: true })
  const close = useCallback(() => setOpen(false), [])
  useDismiss([trigger, list], open, close)

  const enabled = (i: number) => options[i] !== undefined && !options[i]?.disabled
  const move = (from: number, step: 1 | -1) => {
    for (let i = 1; i <= options.length; i++) {
      const next = (from + step * i + options.length) % options.length
      if (enabled(next)) return next
    }
    return from
  }
  const openList = () => {
    const index = options.findIndex((o) => isSelected(o.value))
    setActive(index >= 0 ? index : move(-1, 1))
    setOpen(true)
  }
  const pick = (index: number) => {
    const option = options[index]
    if (!option || option.disabled) return
    if (onPick(option.value)) {
      setOpen(false)
      trigger.current?.focus()
    }
  }

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
        event.preventDefault()
        openList()
      }
      return
    }
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        setActive((a) => move(a, 1))
        break
      case 'ArrowUp':
        event.preventDefault()
        setActive((a) => move(a, -1))
        break
      case 'Home':
        event.preventDefault()
        setActive(move(-1, 1))
        break
      case 'End':
        event.preventDefault()
        setActive(move(options.length, -1))
        break
      case 'Enter':
      case ' ':
        event.preventDefault()
        pick(active)
        break
      case 'Escape':
        event.preventDefault()
        setOpen(false)
        break
      case 'Tab':
        setOpen(false)
        break
      default:
        if (event.key.length === 1) {
          const ch = event.key.toLowerCase()
          const starts = (o: SelectOption<T>, i: number) => enabled(i) && textOf(o).toLowerCase().startsWith(ch)
          const hit = options.findIndex((o, i) => i > active && starts(o, i))
          const wrap = hit >= 0 ? hit : options.findIndex(starts)
          if (wrap >= 0) setActive(wrap)
        }
    }
  }

  const listId = `${id}-list`
  return (
    <div className={cx('g-field', className)} style={style}>
      <span id={`${id}-label`} className={cx('g-label', hideLabel && 'g-visually-hidden')}>
        {label}
      </span>
      <button
        ref={trigger}
        type="button"
        role="combobox"
        className="g-input g-select"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-labelledby={`${id}-label`}
        {...(help ? { 'aria-describedby': `${id}-help` } : {})}
        {...(open && active >= 0 ? { 'aria-activedescendant': `${id}-opt-${active}` } : {})}
        {...(disabled ? { 'data-disabled': '' } : {})}
        onClick={() => (open ? setOpen(false) : openList())}
        onKeyDown={onKeyDown}
      >
        <span className="ui-select-value">{display}</span>
        <ChevronDown size={14} aria-hidden />
      </button>
      {help && (
        <span id={`${id}-help`} className="g-help">
          {help}
        </span>
      )}
      {open && (
        <Portal>
          <div className="ui-popover" style={popover}>
            <ul
              ref={list}
              id={listId}
              role="listbox"
              aria-labelledby={`${id}-label`}
              aria-multiselectable={multiple || undefined}
              className="g-menu ui-listbox"
            >
              {options.map((option, i) => {
                const selected = isSelected(option.value)
                return (
                  <li
                    key={option.value}
                    id={`${id}-opt-${i}`}
                    role="option"
                    aria-selected={selected}
                    aria-disabled={option.disabled || undefined}
                    className="g-menu-item"
                    {...(i === active ? { 'data-active': '' } : {})}
                    onPointerEnter={() => enabled(i) && setActive(i)}
                    // Keep the focus on the field.
                    onPointerDown={(event) => event.preventDefault()}
                    onClick={() => pick(i)}
                  >
                    {multiple && (
                      <Check size={14} aria-hidden style={{ visibility: selected ? 'visible' : 'hidden' }} />
                    )}
                    {option.label}
                    {!multiple && selected && <Check size={14} aria-hidden style={{ marginLeft: 'auto' }} />}
                  </li>
                )
              })}
            </ul>
          </div>
        </Portal>
      )}
    </div>
  )
}
