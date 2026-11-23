import { Pencil } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useLocation } from 'react-router'
import type { DerivedMetric } from '../api/client'
import { derivedReference } from '../dsl/syntax'
import { UnitLabel } from '../metrics/UnitLabel'
import { useUnits } from '../metrics/queries'
import { CopyButton } from '../pages/CopyButton'
import { Facts } from '../pages/Facts'
import { Section } from '../pages/Section'
import { Time } from '../pages/Time'
import { Alert, LinkButton, Stack, TextLink } from '../ui'
import { DerivedMetricActions } from './DerivedMetricActions'
import {
  type DerivedView,
  derivedUsers,
  type SetDerivedView,
} from './derivedModel'
import { derivedHref, derivedMetricHref } from './paths'
import './derived.css'

/** Set by the editor and the actions when they navigate after a change. */
export type SavedState = {
  saved?: { name: string; created: boolean }
  renamed?: { from: string; name: string; rewritten: string[] }
  /** Name of a derived metric that was deleted. */
  deleted?: string
}

type Props = {
  metric: DerivedMetric
  metrics: readonly DerivedMetric[]
  view: DerivedView
  setView: SetDerivedView
}

/**
 * One derived metric (FR-17): its definition, the derived metrics it uses
 * and those that use it. It has no versions (ADR 0017): "Edit" changes the
 * definition, for every analysis and for the metrics built on it. It can
 * also be renamed, and deleted when no other derived metric uses it.
 */
export function DerivedMetricDetail({ metric, metrics, view, setView }: Props) {
  const { t } = useTranslation()
  const location = useLocation()
  const { saved, renamed } = (location.state as SavedState | null) ?? {}
  const units = useUnits()
  const catalogue = useMemo(
    () => units.data && new Map(units.data.map((u) => [u.symbol, u])),
    [units.data],
  )
  const users = useMemo(
    () => derivedUsers(metrics, metric.name),
    [metrics, metric.name],
  )
  const reference = derivedReference(metric.name)

  const editHref = derivedHref({
    name: metric.name,
    edit: 'change',
    run: view.run,
    caseExecution: view.caseExecution,
  })

  return (
    <Section
      title={metric.name}
      actions={
        <Stack row wrap gap={2}>
          <DerivedMetricActions
            metric={metric}
            metrics={metrics}
            view={view}
            setView={setView}
          />
          <LinkButton to={editHref} size="sm" icon={Pencil}>
            {t('derived.detail.edit')}
          </LinkButton>
        </Stack>
      }
    >
      <Stack gap={4}>
        {saved?.name === metric.name && (
          <Alert tone="success">
            {saved.created
              ? t('derived.detail.created')
              : t('derived.detail.changed')}
          </Alert>
        )}
        {renamed?.name === metric.name && (
          <Alert tone="success">
            {t('derived.detail.renamed', { from: renamed.from })}
            {renamed.rewritten.length > 0 &&
              ` ${t('derived.detail.rewritten', {
                count: renamed.rewritten.length,
                names: renamed.rewritten.join(', '),
              })}`}
          </Alert>
        )}
        <div>
          <Stack row justify="space-between" gap={2}>
            <h3 className="body-medium ui-heading">
              {t('derived.detail.expression')}
            </h3>
            <CopyButton
              text={metric.expression}
              label={t('derived.detail.copyExpression')}
            />
          </Stack>
          <pre className="tr-derived-expression">{metric.expression}</pre>
        </div>
        <Facts
          items={[
            {
              label: t('derived.detail.unit'),
              value: <UnitLabel symbol={metric.unit} catalogue={catalogue} />,
            },
            {
              label: t('derived.detail.reference'),
              value: (
                <span className="ui-inline-row">
                  <code className="ui-code">{reference}</code>
                  <CopyButton
                    text={reference}
                    label={t('derived.detail.copyReference')}
                  />
                </span>
              ),
            },
            {
              label: t('derived.detail.description'),
              value: metric.description || t('common.none'),
            },
            {
              label: t('derived.detail.lastChanged'),
              value: <Time iso={metric.updated_at} />,
            },
            {
              label: t('derived.detail.uses'),
              value:
                metric.uses.length === 0 ? (
                  t('derived.detail.usesNone')
                ) : (
                  <MetricLinks names={metric.uses} />
                ),
            },
            {
              label: t('derived.detail.usedBy'),
              value:
                users.length === 0 ? (
                  t('derived.detail.usedByNone')
                ) : (
                  <MetricLinks names={users} />
                ),
            },
          ]}
        />
      </Stack>
    </Section>
  )
}

/** Links to derived metrics, by name. */
function MetricLinks({ names }: { names: readonly string[] }) {
  return (
    <Stack
      row
      wrap
      style={{ columnGap: 'var(--space-3)', rowGap: 'var(--space-1)' }}
    >
      {names.map((name) => (
        <TextLink key={name} to={derivedMetricHref(name)} className="ui-code">
          {name}
        </TextLink>
      ))}
    </Stack>
  )
}
