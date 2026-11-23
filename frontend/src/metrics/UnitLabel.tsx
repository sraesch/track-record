import { useTranslation } from 'react-i18next'
import type { Unit } from '../api/client'
import { Chip, Tooltip } from '../ui'

type Props = {
  symbol: string
  /** The catalogue by symbol; undefined while it loads. */
  catalogue: ReadonlyMap<string, Unit> | undefined
}

/**
 * A unit symbol. Catalogue units name their unit and quantity on hover;
 * others are marked as custom units, which are not converted (FR-13a).
 */
export function UnitLabel({ symbol, catalogue }: Props) {
  const { t } = useTranslation()
  const unit = catalogue?.get(symbol)
  const text =
    symbol === '' ? (
      <span className="ui-secondary">{t('metrics.unit.dimensionless')}</span>
    ) : (
      <code className="ui-code">{symbol}</code>
    )

  if (unit) {
    return (
      <Tooltip
        title={t('metrics.unit.known', {
          name: unit.name,
          quantity: unit.quantity,
        })}
      >
        <span>{text}</span>
      </Tooltip>
    )
  }
  return (
    <span className="ui-inline-row" style={{ gap: 'var(--space-2)' }}>
      {text}
      {catalogue && (
        <Tooltip title={t('metrics.unit.customHint')}>
          <Chip tone="warning" outlined>
            {t('metrics.unit.custom')}
          </Chip>
        </Tooltip>
      )}
    </span>
  )
}
