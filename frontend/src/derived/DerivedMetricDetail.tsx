import EditIcon from '@mui/icons-material/Edit'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Link from '@mui/material/Link'
import Typography from '@mui/material/Typography'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useLocation } from 'react-router'
import type { DerivedMetric } from '../api/client'
import { derivedReference } from '../dsl/syntax'
import { UnitLabel } from '../metrics/UnitLabel'
import { useUnits } from '../metrics/queries'
import { CopyButton } from '../pages/CopyButton'
import { Facts } from '../pages/Facts'
import { Section } from '../pages/Section'
import { Time } from '../pages/Time'
import { DerivedMetricActions } from './DerivedMetricActions'
import {
  type DerivedView,
  derivedUsers,
  type SetDerivedView,
} from './derivedModel'
import { derivedHref, derivedMetricHref } from './paths'

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
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
          <DerivedMetricActions
            metric={metric}
            metrics={metrics}
            view={view}
            setView={setView}
          />
          <Button
            component={RouterLink}
            to={editHref}
            size="small"
            variant="outlined"
            startIcon={<EditIcon />}
          >
            {t('derived.detail.edit')}
          </Button>
        </Box>
      }
    >
      <Box sx={{ display: 'grid', gap: 2 }}>
        {saved?.name === metric.name && (
          <Alert severity="success">
            {saved.created
              ? t('derived.detail.created')
              : t('derived.detail.changed')}
          </Alert>
        )}
        {renamed?.name === metric.name && (
          <Alert severity="success">
            {t('derived.detail.renamed', { from: renamed.from })}
            {renamed.rewritten.length > 0 &&
              ` ${t('derived.detail.rewritten', {
                count: renamed.rewritten.length,
                names: renamed.rewritten.join(', '),
              })}`}
          </Alert>
        )}
        <Box>
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <Typography variant="subtitle2" component="h3">
              {t('derived.detail.expression')}
            </Typography>
            <CopyButton
              text={metric.expression}
              label={t('derived.detail.copyExpression')}
            />
          </Box>
          <Box
            component="pre"
            sx={{
              m: 0,
              p: 1.5,
              borderRadius: 1,
              bgcolor: 'action.hover',
              fontFamily: 'monospace',
              fontSize: '0.875rem',
              whiteSpace: 'pre-wrap',
              overflowWrap: 'anywhere',
            }}
          >
            {metric.expression}
          </Box>
        </Box>
        <Facts
          items={[
            {
              label: t('derived.detail.unit'),
              value: <UnitLabel symbol={metric.unit} catalogue={catalogue} />,
            },
            {
              label: t('derived.detail.reference'),
              value: (
                <Box
                  component="span"
                  sx={{ display: 'inline-flex', alignItems: 'center' }}
                >
                  <Box component="code" sx={{ fontFamily: 'monospace' }}>
                    {reference}
                  </Box>
                  <CopyButton
                    text={reference}
                    label={t('derived.detail.copyReference')}
                  />
                </Box>
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
      </Box>
    </Section>
  )
}

/** Links to derived metrics, by name. */
function MetricLinks({ names }: { names: readonly string[] }) {
  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', columnGap: 1.5, rowGap: 0.5 }}>
      {names.map((name) => (
        <Link
          key={name}
          component={RouterLink}
          to={derivedMetricHref(name)}
          sx={{ fontFamily: 'monospace' }}
        >
          {name}
        </Link>
      ))}
    </Box>
  )
}
