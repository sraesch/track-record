import EditIcon from '@mui/icons-material/Edit'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import Link from '@mui/material/Link'
import List from '@mui/material/List'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemText from '@mui/material/ListItemText'
import Typography from '@mui/material/Typography'
import { type ReactNode, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useLocation } from 'react-router'
import type { DerivedMetricVersion } from '../api/client'
import { derivedReference } from '../dsl/syntax'
import { UnitLabel } from '../metrics/UnitLabel'
import { useUnits } from '../metrics/queries'
import { CopyButton } from '../pages/CopyButton'
import { Facts } from '../pages/Facts'
import { Section } from '../pages/Section'
import { Time } from '../pages/Time'
import {
  type DerivedRow,
  type DerivedView,
  derivedUsers,
  latestVersions,
} from './derivedModel'
import { derivedHref, derivedMetricHref } from './paths'

/** Set by the editor when it navigates here after saving. */
export type SavedState = { saved?: { version: number; created: boolean } }

type Props = {
  row: DerivedRow
  rows: readonly DerivedRow[]
  view: DerivedView
}

/**
 * One derived metric (FR-17): the selected version's definition, the
 * versions it uses and the metrics that use it, and its version history.
 */
export function DerivedMetricDetail({ row, rows, view }: Props) {
  const { t } = useTranslation()
  const location = useLocation()
  const saved = (location.state as SavedState | null)?.saved
  const units = useUnits()
  const catalogue = useMemo(
    () => units.data && new Map(units.data.map((u) => [u.symbol, u])),
    [units.data],
  )
  const shown: DerivedMetricVersion | undefined = view.version
    ? row.versions.find((v) => v.version === view.version)
    : row.latest
  const isLatest = shown?.id === row.latest.id
  const latest = useMemo(() => latestVersions(rows), [rows])
  const users = useMemo(() => derivedUsers(rows, row.name), [rows, row.name])
  const reference = derivedReference(row.name)

  const editHref = derivedHref({
    name: row.name,
    version: view.version,
    edit: 'version',
    run: view.run,
    caseExecution: view.caseExecution,
  })

  return (
    <Section
      title={row.name}
      actions={
        <Button
          component={RouterLink}
          to={editHref}
          size="small"
          variant="outlined"
          startIcon={<EditIcon />}
        >
          {t('derived.detail.newVersion')}
        </Button>
      }
    >
      <Box sx={{ display: 'grid', gap: 2 }}>
        {saved && isLatest && saved.version === row.latest.version && (
          <Alert severity={saved.created ? 'success' : 'info'}>
            {saved.created
              ? t('derived.detail.saved', { version: saved.version })
              : t('derived.detail.unchanged', { version: saved.version })}
          </Alert>
        )}
        {!shown ? (
          <Alert severity="warning">
            {t('derived.detail.noVersion', {
              name: row.name,
              version: view.version,
            })}{' '}
            <Link component={RouterLink} to={derivedMetricHref(row.name)}>
              {t('derived.detail.showLatest')}
            </Link>
          </Alert>
        ) : (
          <>
            {!isLatest && (
              <Alert
                severity="info"
                action={
                  <Button
                    color="inherit"
                    size="small"
                    component={RouterLink}
                    to={derivedMetricHref(row.name)}
                  >
                    {t('derived.detail.showLatest')}
                  </Button>
                }
              >
                {t('derived.detail.olderVersion', {
                  version: shown.version,
                  latest: row.latest.version,
                })}
              </Alert>
            )}
            <Definition
              version={shown}
              latest={latest}
              catalogue={catalogue}
              reference={reference}
            />
          </>
        )}

        <Box>
          <Typography variant="subtitle2" component="h3" sx={{ mb: 0.5 }}>
            {t('derived.detail.usedBy')}
          </Typography>
          {users.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              {t('derived.detail.usedByNone')}
            </Typography>
          ) : (
            <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
              {users.map((user) => (
                <MetricLink
                  key={user.name}
                  name={user.name}
                  version={user.version}
                  note={
                    user.outdated
                      ? t('derived.detail.pins', { version: user.pinned })
                      : undefined
                  }
                />
              ))}
            </Box>
          )}
        </Box>

        <Box>
          <Typography variant="subtitle2" component="h3" sx={{ mb: 0.5 }}>
            {t('derived.detail.history')}
          </Typography>
          <List
            dense
            disablePadding
            aria-label={t('derived.detail.historyLabel', { name: row.name })}
            sx={{ border: 1, borderColor: 'divider', borderRadius: 1 }}
          >
            {row.versions.map((version) => {
              const current = version.id === shown?.id
              return (
                <ListItemButton
                  key={version.id}
                  component={RouterLink}
                  to={derivedHref({
                    name: row.name,
                    version:
                      version.id === row.latest.id ? undefined : version.version,
                    run: view.run,
                    caseExecution: view.caseExecution,
                  })}
                  selected={current}
                  aria-current={current ? 'true' : undefined}
                >
                  <ListItemText
                    primary={
                      <Box
                        component="span"
                        sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}
                      >
                        <span>
                          {t('metrics.version', { version: version.version })}
                        </span>
                        {version.id === row.latest.id && (
                          <Chip
                            size="small"
                            label={t('derived.detail.latest')}
                            sx={{ height: 20 }}
                          />
                        )}
                        <Typography
                          component="span"
                          variant="body2"
                          color="text.secondary"
                        >
                          <Time iso={version.created_at} />
                        </Typography>
                      </Box>
                    }
                    secondary={
                      version.unit
                        ? t('derived.detail.historyEntry', {
                            expression: version.expression,
                            unit: version.unit,
                          })
                        : version.expression
                    }
                    slotProps={{
                      secondary: {
                        noWrap: true,
                        sx: { fontFamily: 'monospace', fontSize: '0.75rem' },
                      },
                    }}
                  />
                </ListItemButton>
              )
            })}
          </List>
        </Box>
      </Box>
    </Section>
  )
}

function Definition({
  version,
  latest,
  catalogue,
  reference,
}: {
  version: DerivedMetricVersion
  latest: ReadonlyMap<string, number>
  catalogue: Parameters<typeof UnitLabel>[0]['catalogue']
  reference: string
}) {
  const { t } = useTranslation()
  return (
    <>
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
            text={version.expression}
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
          {version.expression}
        </Box>
      </Box>
      <Facts
        items={[
          {
            label: t('derived.detail.unit'),
            value: <UnitLabel symbol={version.unit} catalogue={catalogue} />,
          },
          {
            label: t('derived.detail.version'),
            value: t('metrics.version', { version: version.version }),
          },
          {
            label: t('derived.detail.defined'),
            value: <Time iso={version.created_at} />,
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
            value: version.description || t('common.none'),
          },
          {
            label: t('derived.detail.uses'),
            value:
              version.uses.length === 0 ? (
                t('derived.detail.usesNone')
              ) : (
                <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
                  {version.uses.map((used) => {
                    const newest = latest.get(used.name)
                    return (
                      <MetricLink
                        key={used.version_id}
                        name={used.name}
                        version={used.version}
                        pinned
                        note={
                          newest !== undefined && newest > used.version
                            ? t('derived.detail.latestIs', { version: newest })
                            : undefined
                        }
                      />
                    )
                  })}
                </Box>
              ),
          },
        ]}
      />
    </>
  )
}

/**
 * A link to a derived metric, `name v2`. `pinned` links to that exact
 * version; otherwise to the metric. A note flags an outdated pin.
 */
function MetricLink({
  name,
  version,
  pinned,
  note,
}: {
  name: string
  version: number
  pinned?: boolean
  note?: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <Box
      component="span"
      sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.5 }}
    >
      <Link
        component={RouterLink}
        to={derivedMetricHref(name, pinned ? version : undefined)}
        sx={{ fontFamily: 'monospace' }}
      >
        {t('derived.detail.metricVersion', { name, version })}
      </Link>
      {note && (
        <Chip size="small" color="warning" variant="outlined" label={note} />
      )}
    </Box>
  )
}
