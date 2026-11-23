// WAI-ARIA combobox and listbox pattern: the options are <li role="option">
// in a <ul role="listbox">; the focus stays on the field, which handles the
// keys (aria-activedescendant), so the options only take clicks.
/* oxlint-disable jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role, jsx-a11y/click-events-have-key-events */
import { cx } from '@graphite/react'
import { ChevronDown, X } from 'lucide-react'
import {
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
  useCallback,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useTranslation } from 'react-i18next'
import { Portal } from './Popover'
import { useDismiss, usePopoverStyle } from './usePopover'

// Editable comboboxes (WAI-ARIA combobox with a list box): the input is the
// combobox, its list is rendered into <body>. Three kinds, as the app needs
// them: pick one of the options (Combobox), type free text with suggestions
// (SuggestField) and enter several values as chips (TagField).

type FieldProps = {
  /** Visible label above the field. */
  label: string
  hideLabel?: boolean
  required?: boolean
  disabled?: boolean
  /** Below the field; in the error color when `invalid`. */
  help?: ReactNode
  invalid?: boolean
  placeholder?: string
  /** Values and options in monospace (names from uploaded data). */
  mono?: boolean
  className?: string
  style?: CSSProperties
  inputRef?: Ref<HTMLInputElement>
}

type OptionProps<T> = {
  options: readonly T[]
  /** Text of an option, for the field and filtering. Default: String(option). */
  getOptionLabel?: (option: T) => string
  /** Key of an option. Default: its label. */
  getOptionKey?: (option: T) => string
  /** Content of an option in the list. Default: its label. */
  renderOption?: (option: T) => ReactNode
  /** Options to show for the typed text. Default: labels containing it, ignoring case. */
  filterOptions?: (options: readonly T[], text: string) => readonly T[]
  /** The options are being loaded. */
  loading?: boolean
}

function setRef<T>(ref: Ref<T> | undefined, node: T | null) {
  if (typeof ref === 'function') ref(node)
  else if (ref) ref.current = node
}

const containsText = <T,>(label: (o: T) => string) => (options: readonly T[], text: string) => {
  const needle = text.trim().toLowerCase()
  return needle ? options.filter((o) => label(o).toLowerCase().includes(needle)) : options
}

/** Field chrome shared by the comboboxes: label, input box, help. */
function Field({
  id,
  label,
  hideLabel,
  required,
  help,
  invalid,
  disabled,
  className,
  style,
  boxClassName,
  boxRef,
  children,
}: Pick<FieldProps, 'label' | 'hideLabel' | 'required' | 'help' | 'invalid' | 'disabled' | 'className' | 'style'> & {
  id: string
  boxClassName?: string
  boxRef?: Ref<HTMLSpanElement>
  children: ReactNode
}) {
  return (
    <div className={cx('g-field', className)} style={style}>
      <label htmlFor={id} className={cx('g-label', hideLabel && 'g-visually-hidden')}>
        {label}
        {required && (
          <span className="ui-required" aria-hidden>
            *
          </span>
        )}
      </label>
      <span
        ref={boxRef}
        className={cx('g-input', 'ui-combobox', boxClassName)}
        {...(invalid ? { 'data-invalid': '' } : {})}
        {...(disabled ? { 'data-disabled': '' } : {})}
      >
        {children}
      </span>
      {help && (
        <span id={`${id}-help`} className={cx('g-help', invalid && 'g-help-error')}>
          {help}
        </span>
      )}
    </div>
  )
}

type ListProps<T> = {
  id: string
  label: string
  anchor: React.RefObject<HTMLElement | null>
  listRef: React.RefObject<HTMLUListElement | null>
  options: readonly T[]
  active: number
  setActive: (index: number) => void
  isSelected: (option: T) => boolean
  keyOf: (option: T) => string
  render: (option: T) => ReactNode
  onPick: (option: T) => void
  loading: boolean
  /** Say "No options" when there are none (pickers; not free text). */
  showEmpty: boolean
  mono?: boolean
}

function OptionList<T>({
  id,
  label,
  anchor,
  listRef,
  options,
  active,
  setActive,
  isSelected,
  keyOf,
  render,
  onPick,
  loading,
  showEmpty,
  mono,
}: ListProps<T>) {
  const { t } = useTranslation()
  const style = usePopoverStyle(anchor, true, { matchWidth: true })
  const empty = options.length === 0
  if (empty && !loading && !showEmpty) return null
  return (
    <Portal>
      <div className="ui-popover" style={style}>
        <ul
          ref={listRef}
          id={`${id}-list`}
          role="listbox"
          aria-label={label}
          className={cx('g-menu', 'ui-listbox', mono && 'ui-code')}
        >
          {empty ? (
            <li className="ui-listbox-empty" role="presentation">
              {loading ? t('ui.loading') : t('ui.noOptions')}
            </li>
          ) : (
            options.map((option, i) => (
              <li
                key={keyOf(option)}
                id={`${id}-opt-${i}`}
                role="option"
                aria-selected={isSelected(option)}
                className="g-menu-item"
                {...(i === active ? { 'data-active': '' } : {})}
                onPointerEnter={() => setActive(i)}
                // Keep the focus in the input.
                onPointerDown={(event) => event.preventDefault()}
                onClick={() => onPick(option)}
              >
                {render(option)}
              </li>
            ))
          )}
        </ul>
      </div>
    </Portal>
  )
}

/** Arrow keys, Enter and Escape on a combobox input with a list. */
function useListKeys<T>(
  open: boolean,
  setOpen: (open: boolean) => void,
  shown: readonly T[],
  active: number,
  setActive: (fn: (a: number) => number) => void,
  pick: (option: T) => void,
) {
  return (event: KeyboardEvent<HTMLInputElement>) => {
    const n = shown.length
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        if (!open) setOpen(true)
        else if (n > 0) setActive((a) => (a + 1) % n)
        return true
      case 'ArrowUp':
        event.preventDefault()
        if (!open) setOpen(true)
        else if (n > 0) setActive((a) => (a <= 0 ? n - 1 : a - 1))
        return true
      case 'Enter': {
        const option = open && active >= 0 ? shown[active] : undefined
        if (option !== undefined) {
          event.preventDefault()
          pick(option)
          return true
        }
        return false
      }
      case 'Escape':
        if (open) {
          event.preventDefault()
          event.stopPropagation()
          setOpen(false)
          return true
        }
        return false
      default:
        return false
    }
  }
}

export type ComboboxProps<T> = FieldProps &
  OptionProps<T> & {
    value: T | null
    onChange: (value: T | null) => void
  }

/**
 * Picks one of many options; typing filters them. Clearing the text clears
 * the value.
 */
export function Combobox<T>({
  value,
  onChange,
  options,
  getOptionLabel = String,
  getOptionKey,
  renderOption,
  filterOptions,
  loading = false,
  inputRef,
  ...field
}: ComboboxProps<T>) {
  const { t } = useTranslation()
  const id = useId()
  const [open, setOpenState] = useState(false)
  const [text, setText] = useState<string | null>(null)
  const [active, setActive] = useState(-1)
  const box = useRef<HTMLSpanElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLUListElement>(null)
  const keyOf = getOptionKey ?? getOptionLabel
  const shown = useMemo(
    () => (text === null ? options : (filterOptions ?? containsText(getOptionLabel))(options, text)),
    [options, text, filterOptions, getOptionLabel],
  )

  const setOpen = useCallback((next: boolean) => {
    setOpenState(next)
    setActive(-1)
    if (!next) setText(null)
  }, [])
  const close = useCallback(() => setOpen(false), [setOpen])
  useDismiss([box, list], open, close)

  const pick = (option: T) => {
    onChange(option)
    setOpen(false)
  }
  const onKeys = useListKeys(open, setOpen, shown, active, setActive, pick)
  const selectedKey = value === null ? undefined : keyOf(value)

  return (
    <Field id={id} {...field} boxRef={box}>
        <input
          ref={(node) => {
            input.current = node
            setRef(inputRef, node)
          }}
          id={id}
          role="combobox"
          autoComplete="off"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={open ? `${id}-list` : undefined}
          aria-invalid={field.invalid || undefined}
          aria-required={field.required || undefined}
          {...(field.help ? { 'aria-describedby': `${id}-help` } : {})}
          {...(open && active >= 0 ? { 'aria-activedescendant': `${id}-opt-${active}` } : {})}
          className={cx(field.mono && 'ui-code')}
          disabled={field.disabled}
          placeholder={field.placeholder}
          value={text ?? (value === null ? '' : getOptionLabel(value))}
          onChange={(event) => {
            const next = event.target.value
            setText(next)
            setOpenState(true)
            setActive(-1)
            if (next === '' && value !== null) onChange(null)
          }}
          onClick={() => !open && setOpen(true)}
          onBlur={() => setOpen(false)}
          onKeyDown={onKeys}
        />
        <button
          type="button"
          tabIndex={-1}
          className="ui-combobox-toggle"
          aria-label={open ? t('ui.close') : t('ui.open')}
          disabled={field.disabled}
          onPointerDown={(event) => event.preventDefault()}
          onClick={() => {
            setOpen(!open)
            input.current?.focus()
          }}
        >
          <ChevronDown size={14} aria-hidden />
        </button>
      {open && (
        <OptionList
          id={id}
          label={field.label}
          anchor={box}
          listRef={list}
          options={shown}
          active={active}
          setActive={setActive}
          isSelected={(o) => keyOf(o) === selectedKey}
          keyOf={keyOf}
          render={renderOption ?? getOptionLabel}
          onPick={pick}
          loading={loading}
          showEmpty
          mono={field.mono}
        />
      )}
    </Field>
  )
}

export type SuggestFieldProps<T> = FieldProps &
  OptionProps<T> & {
    /** The text: the field's value. */
    value: string
    onChange: (text: string) => void
    /** An option was picked; `onChange` has its label already. */
    onSelect?: (option: T) => void
  }

/** Free text, with matching options to pick from as the user types. */
export function SuggestField<T = string>({
  value,
  onChange,
  onSelect,
  options,
  getOptionLabel = String,
  getOptionKey,
  renderOption,
  filterOptions,
  loading = false,
  inputRef,
  ...field
}: SuggestFieldProps<T>) {
  const id = useId()
  const [open, setOpenState] = useState(false)
  const [active, setActive] = useState(-1)
  const box = useRef<HTMLSpanElement>(null)
  const list = useRef<HTMLUListElement>(null)
  const keyOf = getOptionKey ?? getOptionLabel
  const shown = useMemo(
    () => (filterOptions ?? containsText(getOptionLabel))(options, value),
    [options, value, filterOptions, getOptionLabel],
  )

  const setOpen = useCallback((next: boolean) => {
    setOpenState(next)
    setActive(-1)
  }, [])
  const close = useCallback(() => setOpen(false), [setOpen])
  useDismiss([box, list], open, close)

  const pick = (option: T) => {
    onChange(getOptionLabel(option))
    onSelect?.(option)
    setOpen(false)
  }
  const onKeys = useListKeys(open, setOpen, shown, active, setActive, pick)

  return (
    <Field id={id} {...field} boxRef={box}>
        <input
          ref={inputRef}
          id={id}
          role="combobox"
          autoComplete="off"
          aria-autocomplete="list"
          aria-expanded={open && shown.length > 0}
          aria-controls={open && shown.length > 0 ? `${id}-list` : undefined}
          aria-invalid={field.invalid || undefined}
          aria-required={field.required || undefined}
          {...(field.help ? { 'aria-describedby': `${id}-help` } : {})}
          {...(open && active >= 0 ? { 'aria-activedescendant': `${id}-opt-${active}` } : {})}
          className={cx(field.mono && 'ui-code')}
          disabled={field.disabled}
          placeholder={field.placeholder}
          value={value}
          onChange={(event) => {
            onChange(event.target.value)
            setOpenState(true)
            setActive(-1)
          }}
          onClick={() => !open && setOpen(true)}
          onBlur={() => setOpen(false)}
          onKeyDown={onKeys}
        />
      {open && (
        <OptionList
          id={id}
          label={field.label}
          anchor={box}
          listRef={list}
          options={shown}
          active={active}
          setActive={setActive}
          isSelected={(o) => getOptionLabel(o) === value}
          keyOf={keyOf}
          render={renderOption ?? getOptionLabel}
          onPick={pick}
          loading={loading}
          showEmpty={false}
          mono={field.mono}
        />
      )}
    </Field>
  )
}

export type TagFieldProps = FieldProps & {
  values: readonly string[]
  onChange: (values: string[]) => void
  /** The text being typed; uncontrolled when left out. */
  inputValue?: string
  onInputChange?: (text: string) => void
  /**
   * Called with the text when Enter adds it. Return false to keep the text
   * in the field (e.g. to fix it) instead of adding it.
   */
  onAdd?: (text: string) => boolean
}

/**
 * Several free-text values, shown as chips. Enter adds the typed text,
 * Backspace in the empty field removes the last value. The input is a
 * plain text box: there is no list to pick from.
 */
export function TagField({ values, onChange, inputValue, onInputChange, onAdd, inputRef, ...field }: TagFieldProps) {
  const { t } = useTranslation()
  const id = useId()
  const [ownText, setOwnText] = useState('')
  const text = inputValue ?? ownText
  const setText = (next: string) => {
    setOwnText(next)
    onInputChange?.(next)
  }

  return (
    <Field id={id} {...field} boxClassName="ui-tags">
      {values.map((value, index) => (
        <span key={value} className={cx('ui-chip', field.mono && 'ui-chip-mono')}>
          <span className="ui-chip-label">{value}</span>
          {!field.disabled && (
            <button
              type="button"
              tabIndex={-1}
              className="ui-chip-remove"
              aria-label={t('ui.remove', { item: value })}
              onClick={() => onChange(values.filter((_, i) => i !== index))}
            >
              <X size={12} aria-hidden />
            </button>
          )}
        </span>
      ))}
      <input
        ref={inputRef}
        id={id}
        autoComplete="off"
        aria-invalid={field.invalid || undefined}
        {...(field.help ? { 'aria-describedby': `${id}-help` } : {})}
        className={cx(field.mono && 'ui-code')}
        disabled={field.disabled}
        placeholder={field.placeholder}
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            const added = text.trim()
            if (!added) return
            event.preventDefault()
            if (onAdd && !onAdd(text)) return
            if (!values.includes(added)) onChange([...values, added])
            setText('')
          } else if (event.key === 'Backspace' && text === '' && values.length > 0) {
            onChange(values.slice(0, -1))
          }
        }}
      />
    </Field>
  )
}
