import {
  ArrowDown,
  ArrowUp,
  ListChecks,
  Pencil,
  PencilLine,
  Trash2,
} from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Metric, MetricDirection, Unit } from '../api/client'
import { Time } from '../pages/Time'
import { IconButton, IconLinkButton, Table, VisuallyHidden } from '../ui'
import { type MetricAction, MetricDialog } from './MetricDialogs'
import { UnitLabel } from './UnitLabel'

type Props = {
  /** The metrics to show. */
  rows: readonly Metric[]
  /** All metrics, shown or not. */
  metrics: readonly Metric[]
  units: readonly Unit[]
  catalogue: ReadonlyMap<string, Unit> | undefined
}

/**
 * The uploaded metrics with their definition. A metric has one definition
 * and no versions (ADR 0018); it is changed, renamed and deleted here.
 */
export function MetricTable({ rows, metrics, units, catalogue }: Props) {
  const { t } = useTranslation()
  // The dialog keeps its metric while it closes, also when the list has
  // changed underneath it (renamed or deleted).
  const [dialog, setDialog] = useState<{
    action: MetricAction
    metric: Metric
  }>()
  const [open, setOpen] = useState(false)
  const start = (action: MetricAction, metric: Metric) => {
    setDialog({ action, metric })
    setOpen(true)
  }

  return (
    <>
      <Table framed aria-label={t('metrics.tableLabel')}>
        <thead>
          <tr>
            <th>{t('metrics.columns.name')}</th>
            <th>{t('metrics.columns.unit')}</th>
            <th>{t('metrics.columns.valueType')}</th>
            <th>{t('metrics.columns.direction')}</th>
            <th>{t('metrics.columns.description')}</th>
            <th>{t('metrics.columns.changed')}</th>
            <th className="g-r">
              <VisuallyHidden>{t('metrics.columns.actions')}</VisuallyHidden>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((metric) => (
            <tr key={metric.id}>
              <td>
                <code className="mono body-medium">{metric.name}</code>
              </td>
              <td>
                <UnitLabel symbol={metric.unit} catalogue={catalogue} />
              </td>
              <td>{t(`metrics.valueType.${metric.value_type}`)}</td>
              <td>
                <Direction direction={metric.direction} />
              </td>
              <td className="ui-wrap-cell tr-metrics-description">
                {metric.description || (
                  <span className="ui-secondary">{t('common.none')}</span>
                )}
              </td>
              <td>
                <Time iso={metric.updated_at} />
              </td>
              <td className="g-r">
                <IconLinkButton
                  size="sm"
                  icon={ListChecks}
                  to={`/runs?${new URLSearchParams({ metric: metric.name })}`}
                  label={t('metrics.runsNamed', { name: metric.name })}
                  tooltip={t('metrics.runs')}
                />
                <IconButton
                  size="sm"
                  icon={Pencil}
                  onClick={() => start('edit', metric)}
                  label={t('metrics.edit.named', { name: metric.name })}
                  tooltip={t('metrics.edit.action')}
                />
                <IconButton
                  size="sm"
                  icon={PencilLine}
                  onClick={() => start('rename', metric)}
                  label={t('metrics.rename.named', { name: metric.name })}
                  tooltip={t('metrics.rename.action')}
                />
                <IconButton
                  size="sm"
                  icon={Trash2}
                  onClick={() => start('delete', metric)}
                  label={t('metrics.delete.named', { name: metric.name })}
                  tooltip={t('metrics.delete.action')}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </Table>
      {dialog && (
        <MetricDialog
          action={dialog.action}
          metric={dialog.metric}
          metrics={metrics}
          units={units}
          open={open}
          onClose={() => setOpen(false)}
          onExited={() => setDialog(undefined)}
        />
      )}
    </>
  )
}

function Direction({ direction }: { direction: MetricDirection }) {
  const { t } = useTranslation()
  const Icon = direction === 'higher_is_better' ? ArrowUp : ArrowDown
  return (
    <span className="ui-inline-row">
      <Icon size={14} className="ui-secondary" aria-hidden />
      {t(`metrics.direction.${direction}`)}
    </span>
  )
}
