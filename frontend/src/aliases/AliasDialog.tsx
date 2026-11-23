import { SegmentedControl } from '@graphite/react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { Alert, Select, SuggestField } from '../ui'
import {
  type AliasRow,
  type AliasValue,
  deviceDimension,
  parameterDimension,
  parseDimension,
  parseValue,
  resolve,
  sameValue,
  typeOf,
  valueKey,
  valueText,
  type ValueType,
  valueTypes,
} from './aliasModel'
import { type KnownValues, useSetAlias } from './queries'
import { useValueLabel } from './useValueLabel'
import './aliases.css'

/** What the dialog does: add a new alias, or change one that exists. */
export type AliasDialogMode =
  | { kind: 'add' }
  /** Change the target of an alias, or set a removed one again. */
  | { kind: 'edit'; row: AliasRow }

type Props = {
  open: boolean
  mode: AliasDialogMode
  /** All aliases, to tell what a change replaces or chains into. */
  rows: readonly AliasRow[]
  known: KnownValues | undefined
  onClose: () => void
}

type Field = { text: string; type: ValueType }

const emptyField: Field = { text: '', type: 'string' }
const fieldOf = (value: AliasValue | undefined): Field =>
  value === undefined
    ? emptyField
    : { text: valueText(value), type: typeOf(value) }

/**
 * Adds an alias or changes its target (FR-14). Parameter values keep their
 * JSON type, so a parameter value is entered with its type; choosing a
 * value found in the results sets both.
 */
export function AliasDialog({ open, mode, rows, known, onClose }: Props) {
  const { t } = useTranslation()
  const label = useValueLabel()
  const setAlias = useSetAlias()
  const editing = mode.kind === 'edit' ? mode.row : undefined
  const fixed = editing ? parseDimension(editing.dimension) : undefined

  const [kind, setKind] = useState<'device' | 'parameter'>(
    fixed?.kind ?? 'device',
  )
  const [parameterName, setParameterName] = useState(
    fixed?.kind === 'parameter' ? fixed.name : '',
  )
  const [source, setSource] = useState<Field>(fieldOf(editing?.source))
  const [target, setTarget] = useState<Field>(() => {
    if (!editing) return emptyField
    // A removed alias starts empty, with the type of its source.
    return editing.target === undefined
      ? { text: '', type: typeOf(editing.source) }
      : fieldOf(editing.target)
  })
  // After switching to a parameter, its name is what to type next.
  const parameterNameRef = useRef<HTMLInputElement>(null)
  const switchedKind = useRef(false)
  useEffect(() => {
    if (kind === 'parameter' && switchedKind.current)
      parameterNameRef.current?.focus()
  }, [kind])
  // The target follows the source's type until it is chosen on its own.
  const [targetTypeChosen, setTargetTypeChosen] = useState(
    editing !== undefined,
  )

  const dimension = editing
    ? editing.dimension
    : kind === 'device'
      ? deviceDimension
      : parameterName.trim()
        ? parameterDimension(parameterName)
        : undefined
  const isParameter = kind === 'parameter'
  const sourceValue = parseValue(source.text, source.type)
  const targetValue = parseValue(target.text, target.type)
  const same =
    sourceValue !== undefined &&
    targetValue !== undefined &&
    sameValue(sourceValue, targetValue)
  const canSave =
    dimension !== undefined &&
    sourceValue !== undefined &&
    targetValue !== undefined &&
    !same

  const knownValues = dimension ? (known?.values(dimension) ?? []) : []
  const knownTargets = dimension
    ? targetOptions(rows, dimension, knownValues)
    : []

  // Notes on what saving does; only for complete input.
  const notes: string[] = []
  if (
    dimension &&
    sourceValue !== undefined &&
    targetValue !== undefined &&
    !same
  ) {
    const current = rows.find(
      (r) =>
        r.dimension === dimension &&
        r.target !== undefined &&
        sameValue(r.source, sourceValue),
    )
    if (!editing && current?.target !== undefined) {
      notes.push(
        t('aliases.dialog.replaces', {
          target: label(current.target, isParameter),
        }),
      )
    }
    if (
      known &&
      !knownValues.some((v) => valueKey(v) === valueKey(sourceValue))
    ) {
      notes.push(
        t('aliases.dialog.unknownSource', {
          source: label(sourceValue, isParameter),
        }),
      )
    }
    const final = resolve(
      rows.filter(
        (r) => !(r.dimension === dimension && sameValue(r.source, sourceValue)),
      ),
      dimension,
      targetValue,
    )
    if (!sameValue(final, targetValue)) {
      notes.push(
        t('aliases.dialog.chain', {
          target: label(targetValue, isParameter),
          final: label(final, isParameter),
        }),
      )
    }
    if (typeOf(sourceValue) !== typeOf(targetValue)) {
      notes.push(
        t('aliases.dialog.typeChange', {
          from: t(`aliases.type.${typeOf(sourceValue)}`),
          to: t(`aliases.type.${typeOf(targetValue)}`),
        }),
      )
    }
  }

  const title = !editing
    ? t('aliases.dialog.addTitle')
    : editing.target === undefined
      ? t('aliases.dialog.restoreTitle')
      : t('aliases.dialog.editTitle')

  return (
    <ConfirmDialog
      open={open}
      title={title}
      confirmLabel={t('common.save')}
      canConfirm={canSave}
      pending={setAlias.isPending}
      error={setAlias.error}
      onClose={onClose}
      onConfirm={() => {
        if (!canSave) return
        setAlias.mutate(
          { dimension, source: sourceValue, target: targetValue },
          { onSuccess: onClose },
        )
      }}
    >
      <div className="tr-aliases-kind">
        {/* The fieldset disables the buttons: the dimension stays when editing. */}
        <fieldset disabled={editing !== undefined}>
          <SegmentedControl<'device' | 'parameter'>
            aria-label={t('aliases.dialog.kind')}
            items={[
              { value: 'device', label: t('aliases.dimension.device') },
              { value: 'parameter', label: t('aliases.dimension.parameter') },
            ]}
            value={kind}
            onChange={(value) => {
              if (value === kind) return
              switchedKind.current = true
              setKind(value)
              // Device names are text.
              if (value === 'device') {
                setSource((s) => ({ ...s, type: 'string' }))
                setTarget((s) => ({ ...s, type: 'string' }))
                setTargetTypeChosen(false)
              }
            }}
          />
        </fieldset>
        {isParameter && (
          <SuggestField
            className="tr-aliases-name"
            label={t('aliases.dialog.parameterName')}
            required
            mono
            disabled={editing !== undefined}
            inputRef={parameterNameRef}
            options={known?.parameterNames ?? []}
            value={parameterName}
            onChange={setParameterName}
          />
        )}
      </div>

      <ValueField
        label={t('aliases.dialog.source')}
        helperText={t('aliases.dialog.sourceHelper')}
        field={source}
        typed={isParameter}
        disabled={editing !== undefined}
        options={knownValues}
        onChange={(next) => {
          setSource(next)
          if (!targetTypeChosen) setTarget((s) => ({ ...s, type: next.type }))
        }}
      />
      <ValueField
        label={t('aliases.dialog.target')}
        helperText={
          same ? t('aliases.dialog.same') : t('aliases.dialog.targetHelper')
        }
        error={same}
        field={target}
        typed={isParameter}
        options={knownTargets}
        onChange={(next) => {
          if (next.type !== target.type) setTargetTypeChosen(true)
          setTarget(next)
        }}
      />

      {notes.map((note) => (
        <Alert key={note} tone="info">
          {note}
        </Alert>
      ))}
    </ConfirmDialog>
  )
}

/**
 * Values to merge into: the uploaded values and the targets of other
 * aliases of the dimension (a name only aliases produce), each once.
 */
function targetOptions(
  rows: readonly AliasRow[],
  dimension: string,
  known: readonly AliasValue[],
): AliasValue[] {
  const all = new Map(known.map((v) => [valueKey(v), v]))
  for (const row of rows) {
    if (row.dimension === dimension && row.target !== undefined) {
      all.set(valueKey(row.target), row.target)
    }
  }
  return [...all.values()].sort((a, b) =>
    String(a).localeCompare(String(b), undefined, { numeric: true }),
  )
}

type ValueFieldProps = {
  label: string
  helperText: string
  error?: boolean
  field: Field
  /** Show the type: parameter values keep their JSON type. */
  typed: boolean
  disabled?: boolean
  options: readonly AliasValue[]
  onChange: (field: Field) => void
}

/** A value with its type; values from the results can be picked. */
function ValueField({
  label,
  helperText,
  error,
  field,
  typed,
  disabled,
  options,
  onChange,
}: ValueFieldProps) {
  const { t } = useTranslation()
  const valueLabel = useValueLabel()
  const invalid =
    field.text.trim() !== '' && parseValue(field.text, field.type) === undefined
  const shownHelper = invalid
    ? field.type === 'number'
      ? t('aliases.dialog.invalidNumber')
      : t('aliases.dialog.invalidBoolean')
    : helperText

  return (
    <div className="tr-aliases-field">
      <SuggestField<AliasValue>
        className="ui-grow"
        label={label}
        required
        disabled={disabled}
        help={shownHelper}
        invalid={error || invalid}
        options={options}
        getOptionLabel={valueText}
        getOptionKey={valueKey}
        renderOption={(option) => valueLabel(option, typed)}
        // Free text arrives through onChange, a picked value with its type
        // through onSelect (after onChange). Device names are strings too:
        // only a picked option brings its type; typed text keeps the
        // chosen one.
        value={field.text}
        onChange={(text) => onChange({ ...field, text })}
        onSelect={(option) =>
          onChange({ text: valueText(option), type: typeOf(option) })
        }
      />
      {typed && (
        <Select<ValueType>
          className="tr-aliases-type"
          disabled={disabled}
          label={t('aliases.dialog.type')}
          options={valueTypes.map((type) => ({
            value: type,
            label: t(`aliases.type.${type}`),
          }))}
          value={field.type}
          onChange={(type) => onChange({ ...field, type })}
        />
      )}
    </div>
  )
}
