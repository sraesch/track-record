import { TextField } from '@graphite/react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type {
  Metric,
  MetricDirection,
  MetricValueType,
  Unit,
} from '../api/client'
import { quoteName } from '../dsl/syntax'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { Alert, Select, SuggestField, TextArea } from '../ui'
import { deleteBlocker, isValidMetricName } from './metricModel'
import { useDeleteMetric, useRenameMetric, useSaveMetric } from './queries'

export type MetricAction = 'edit' | 'rename' | 'delete'

type Props = {
  /** What to do with `metric`; the dialog of this action is the one shown. */
  action: MetricAction
  metric: Metric
  /** All metrics, to tell that a new name is taken. */
  metrics: readonly Metric[]
  units: readonly Unit[]
  open: boolean
  onClose: () => void
  /** Called once the dialog has closed. */
  onExited: () => void
}

/**
 * "Edit", "Rename" and "Delete" for an uploaded metric, each asking first
 * (ADR 0018). A metric has one definition: what is changed here applies to
 * all its results.
 */
export function MetricDialog(props: Props) {
  switch (props.action) {
    case 'edit':
      return <EditDialog {...props} />
    case 'rename':
      return <RenameDialog {...props} />
    case 'delete':
      return <DeleteDialog {...props} />
  }
}

const valueTypes: MetricValueType[] = ['float', 'integer', 'boolean']
const directions: MetricDirection[] = ['higher_is_better', 'lower_is_better']

const code = (text: string) => <code className="ui-code">{text}</code>

function EditDialog({ metric, units, open, onClose, onExited }: Props) {
  const { t } = useTranslation()
  const [unit, setUnit] = useState(metric.unit)
  const [valueType, setValueType] = useState(metric.value_type)
  const [direction, setDirection] = useState(metric.direction)
  const [description, setDescription] = useState(metric.description)
  const save = useSaveMetric()

  const unitSymbols = useMemo(() => units.map((u) => u.symbol), [units])
  const newUnit = unit.trim()
  const unitChanged = newUnit !== metric.unit
  const reinterprets =
    metric.has_results && (unitChanged || valueType !== metric.value_type)

  return (
    <ConfirmDialog
      open={open}
      title={t('metrics.edit.title', { name: metric.name })}
      confirmLabel={t('common.save')}
      pending={save.isPending}
      error={save.error}
      onClose={onClose}
      onExited={onExited}
      onConfirm={() =>
        save.mutate(
          {
            name: metric.name,
            body: {
              unit: newUnit,
              value_type: valueType,
              direction,
              description: description.trim(),
            },
          },
          { onSuccess: onClose },
        )
      }
    >
      <p className="ui-text">{t('metrics.edit.text')}</p>
      <SuggestField
        label={t('metrics.columns.unit')}
        help={t('metrics.edit.unitHelp')}
        mono
        options={unitSymbols}
        value={unit}
        onChange={setUnit}
      />
      <Select
        label={t('metrics.columns.valueType')}
        options={valueTypes.map((type) => ({
          value: type,
          label: t(`metrics.valueType.${type}`),
        }))}
        value={valueType}
        onChange={setValueType}
      />
      <Select
        label={t('metrics.columns.direction')}
        options={directions.map((d) => ({
          value: d,
          label: t(`metrics.direction.${d}`),
        }))}
        value={direction}
        onChange={setDirection}
      />
      <TextArea
        label={t('metrics.columns.description')}
        value={description}
        onChange={(event) => setDescription(event.target.value)}
        rows={2}
      />
      {reinterprets && (
        <Alert tone="warning">{t('metrics.edit.reinterprets')}</Alert>
      )}
      {unitChanged && metric.used_by.length > 0 && (
        <Alert tone="info">
          {t('metrics.edit.users', { count: metric.used_by.length })}{' '}
          {code(metric.used_by.join(', '))}
        </Alert>
      )}
    </ConfirmDialog>
  )
}

function RenameDialog({ metric, metrics, open, onClose, onExited }: Props) {
  const { t } = useTranslation()
  const [newName, setNewName] = useState(metric.name)
  const rename = useRenameMetric()

  const error =
    newName === ''
      ? undefined
      : !isValidMetricName(newName)
        ? t('metrics.rename.invalid')
        : newName === metric.name
          ? t('metrics.rename.same')
          : metrics.some((m) => m.name === newName)
            ? t('metrics.rename.taken')
            : undefined

  return (
    <ConfirmDialog
      open={open}
      title={t('metrics.rename.title', { name: metric.name })}
      confirmLabel={t('metrics.rename.confirm')}
      canConfirm={newName.trim() !== '' && error === undefined}
      pending={rename.isPending}
      error={rename.error}
      onClose={onClose}
      onExited={onExited}
      onConfirm={() =>
        rename.mutate({ name: metric.name, newName }, { onSuccess: onClose })
      }
    >
      <TextField
        label={t('metrics.rename.newName')}
        className="ui-mono-input"
        value={newName}
        onChange={(event) => setNewName(event.target.value)}
        error={error}
        help={error === undefined ? t('metrics.rename.help') : undefined}
        spellCheck={false}
        autoComplete="off"
      />
      {metric.used_by.length > 0 && (
        <p className="ui-text">
          {t('metrics.rename.users', { count: metric.used_by.length })}{' '}
          {code(metric.used_by.join(', '))}
        </p>
      )}
      <Alert tone="warning">
        {t('metrics.rename.harness')} {code(metric.name)}
      </Alert>
      <Alert tone="warning">
        {t('metrics.rename.views')} {code(quoteName(metric.name))}
      </Alert>
    </ConfirmDialog>
  )
}

function DeleteDialog({ metric, open, onClose, onExited }: Props) {
  const { t } = useTranslation()
  const remove = useDeleteMetric()
  const blocker = deleteBlocker(metric)

  return (
    <ConfirmDialog
      open={open}
      title={t('metrics.delete.title', { name: metric.name })}
      confirmLabel={t('common.delete')}
      destructive
      canConfirm={blocker === undefined}
      pending={remove.isPending}
      error={remove.error}
      onClose={onClose}
      onExited={onExited}
      onConfirm={() => remove.mutate(metric.name, { onSuccess: onClose })}
    >
      {blocker === 'results' ? (
        <Alert tone="info">{t('metrics.delete.hasResults')}</Alert>
      ) : blocker === 'derived' ? (
        <Alert tone="info">
          {t('metrics.delete.inUse', { count: metric.used_by.length })}{' '}
          {code(metric.used_by.join(', '))}
        </Alert>
      ) : (
        <p className="ui-text">{t('metrics.delete.text')}</p>
      )}
    </ConfirmDialog>
  )
}
