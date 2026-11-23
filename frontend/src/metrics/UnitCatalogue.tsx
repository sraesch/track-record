import { useTranslation } from 'react-i18next'
import { formatNumber } from '../format'
import { Section } from '../pages/Section'
import { Chip, ChipLink, Table } from '../ui'
import { metricsViewToParams, type QuantityGroup } from './metricModel'
import './metrics.css'

type Props = {
  groups: readonly QuantityGroup[]
  /** Custom units used by metrics (not converted), shown first. */
  custom: readonly { symbol: string; metrics: string[] }[]
  /** Metric names per unit symbol (latest versions). */
  usedBy: ReadonlyMap<string, string[]>
}

/**
 * The predefined units (FR-13a), one section per quantity. Values convert
 * between the units of one quantity through its base unit.
 */
export function UnitCatalogue({ groups, custom, usedBy }: Props) {
  const { t } = useTranslation()
  return (
    <div className="tr-metrics-units">
      {custom.length > 0 && (
        <Section title={t('metrics.units.customTitle')} flush>
          <p className="ui-text ui-secondary small tr-metrics-units-text">
            {t('metrics.units.customText')}
          </p>
          <Table aria-label={t('metrics.units.customTitle')}>
            <thead>
              <tr>
                <th>{t('metrics.units.columns.symbol')}</th>
                <th>{t('metrics.units.columns.usedBy')}</th>
              </tr>
            </thead>
            <tbody>
              {custom.map((unit) => (
                <tr key={unit.symbol}>
                  <td>
                    <Symbol symbol={unit.symbol} />
                  </td>
                  <td className="ui-wrap-cell">
                    <MetricLinks names={unit.metrics} />
                  </td>
                </tr>
              ))}
            </tbody>
          </Table>
        </Section>
      )}

      {groups.map((group) => (
        <Section
          key={group.quantity}
          title={capitalize(group.quantity)}
          flush
          actions={
            group.base && (
              <span className="ui-secondary small">
                {t('metrics.units.baseUnit', {
                  unit: group.base.symbol || t('metrics.unit.dimensionless'),
                })}
              </span>
            )
          }
        >
          <Table aria-label={capitalize(group.quantity)}>
            <thead>
              <tr>
                <th>{t('metrics.units.columns.symbol')}</th>
                <th>{t('metrics.units.columns.name')}</th>
                <th className="g-r">{t('metrics.units.columns.inBase')}</th>
                <th>{t('metrics.units.columns.usedBy')}</th>
              </tr>
            </thead>
            <tbody>
              {group.units.map((unit) => (
                <tr key={unit.symbol}>
                  <td>
                    <Symbol symbol={unit.symbol} />
                  </td>
                  <td className="ui-wrap-cell">{unit.name}</td>
                  <td className="g-r">
                    {unit === group.base ? (
                      <Chip outlined>{t('metrics.units.base')}</Chip>
                    ) : group.base ? (
                      t('common.valueWithUnit', {
                        value: formatNumber(unit.to_base),
                        unit: group.base.symbol,
                      }).trim()
                    ) : (
                      formatNumber(unit.to_base)
                    )}
                  </td>
                  <td className="ui-wrap-cell">
                    <MetricLinks names={usedBy.get(unit.symbol) ?? []} />
                  </td>
                </tr>
              ))}
            </tbody>
          </Table>
        </Section>
      ))}
    </div>
  )
}

function Symbol({ symbol }: { symbol: string }) {
  const { t } = useTranslation()
  return symbol === '' ? (
    <span className="ui-secondary">{t('metrics.unit.dimensionless')}</span>
  ) : (
    <code className="ui-code">{symbol}</code>
  )
}

/** The metrics using a unit; each opens the metric list searched for it. */
function MetricLinks({ names }: { names: readonly string[] }) {
  const { t } = useTranslation()
  if (names.length === 0) {
    return <span className="ui-secondary">{t('common.none')}</span>
  }
  return (
    <span className="tr-metrics-links">
      {names.map((name) => (
        <ChipLink
          key={name}
          to={`/metrics?${metricsViewToParams({ tab: 'metrics', search: name })}`}
          aria-label={t('metrics.units.showMetric', { name })}
        >
          {name}
        </ChipLink>
      ))}
    </span>
  )
}

/** Quantities come from the catalogue in lower case ("data size"). */
const capitalize = (text: string) =>
  text.charAt(0).toLocaleUpperCase() + text.slice(1)
