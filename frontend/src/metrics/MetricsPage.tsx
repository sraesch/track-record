import Box from '@mui/material/Box'
import LinearProgress from '@mui/material/LinearProgress'
import Tab from '@mui/material/Tab'
import Tabs from '@mui/material/Tabs'
import Typography from '@mui/material/Typography'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
import {
  customUnits,
  filterMetrics,
  filterUnits,
  groupUnits,
  metricsPerUnit,
  type MetricsTab,
  type MetricsView,
  metricsViewToParams,
  parseMetricsView,
} from './metricModel'
import { MetricTable } from './MetricTable'
import { useMetrics, useUnits } from './queries'
import { UnitCatalogue } from './UnitCatalogue'

const tabId = (tab: MetricsTab) => `metrics-tab-${tab}`
const panelId = (tab: MetricsTab) => `metrics-panel-${tab}`

/**
 * The uploaded metrics (FR-7) with their unit, value type and direction,
 * and the unit catalogue (FR-13a). The harness creates a metric when it
 * first uploads results for it. A metric has one definition and no versions
 * (ADR 0018): it is changed, renamed and deleted here. Tab and search are in
 * the URL (FR-21).
 */
export function MetricsPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const view = useMemo(() => parseMetricsView(searchParams), [searchParams])
  const metrics = useMetrics()
  const units = useUnits()

  // Typing in the search replaces the history entry instead of adding one
  // per character.
  const setView = (next: MetricsView, replace = false) =>
    setSearchParams(metricsViewToParams(next), { replace })

  const catalogue = useMemo(
    () => units.data && new Map(units.data.map((u) => [u.symbol, u])),
    [units.data],
  )
  const groups = useMemo(() => groupUnits(units.data ?? []), [units.data])
  const rows = useMemo(() => metrics.data ?? [], [metrics.data])
  const shownMetrics = useMemo(
    () => filterMetrics(rows, view.search),
    [rows, view.search],
  )
  const shownGroups = useMemo(
    () => filterUnits(groups, view.search),
    [groups, view.search],
  )
  const usedBy = useMemo(() => metricsPerUnit(rows), [rows])
  const custom = useMemo(() => {
    if (!units.data) return []
    const needle = view.search?.toLowerCase()
    return customUnits(rows, units.data).filter(
      (unit) =>
        !needle ||
        unit.symbol.toLowerCase().includes(needle) ||
        unit.metrics.some((name) => name.toLowerCase().includes(needle)),
    )
  }, [rows, units.data, view.search])

  const query = view.tab === 'metrics' ? metrics : units
  const unitCount = shownGroups.reduce((n, g) => n + g.units.length, 0)

  return (
    <>
      <PageHeader
        title={t('nav.items.metrics')}
        description={t('pages.metrics.description')}
      />

      <Box sx={{ borderBottom: 1, borderColor: 'divider', mb: 2 }}>
        <Tabs
          value={view.tab}
          onChange={(_, tab: MetricsTab) => setView({ ...view, tab })}
          aria-label={t('metrics.tabs.label')}
          variant="scrollable"
          allowScrollButtonsMobile
        >
          <Tab
            value="metrics"
            label={t('metrics.tabs.metrics')}
            id={tabId('metrics')}
            aria-controls={panelId('metrics')}
          />
          <Tab
            value="units"
            label={t('metrics.tabs.units')}
            id={tabId('units')}
            aria-controls={panelId('units')}
          />
        </Tabs>
      </Box>

      <Box
        component="search"
        aria-label={t('metrics.searchLabel')}
        sx={{ display: 'flex', mb: 2 }}
      >
        <SearchField
          label={
            view.tab === 'metrics'
              ? t('metrics.searchMetrics')
              : t('metrics.searchUnits')
          }
          value={view.search ?? ''}
          onChange={(search) => setView({ ...view, search }, true)}
        />
      </Box>

      <Box sx={{ height: 4, mb: 1 }}>
        {(metrics.isFetching || units.isFetching) && (
          <LinearProgress aria-label={t('common.loading')} />
        )}
      </Box>

      <Box
        role="tabpanel"
        id={panelId(view.tab)}
        aria-labelledby={tabId(view.tab)}
      >
        {query.isError && !query.data ? (
          <ErrorAlert
            message={t(
              view.tab === 'metrics' ? 'metrics.error' : 'metrics.units.error',
              { message: query.error.message },
            )}
            onRetry={() => query.refetch()}
          />
        ) : !query.data ? null : view.tab === 'metrics' ? (
          shownMetrics.length === 0 ? (
            <EmptyState>
              {rows.length === 0 ? t('metrics.empty') : t('metrics.noMatch')}
            </EmptyState>
          ) : (
            <>
              <MetricTable
                rows={shownMetrics}
                metrics={rows}
                units={units.data ?? []}
                catalogue={catalogue}
              />
              <Typography
                variant="body2"
                color="text.secondary"
                sx={{ my: 1.5 }}
              >
                {t('metrics.count', { count: shownMetrics.length })}
              </Typography>
            </>
          )
        ) : shownGroups.length === 0 && custom.length === 0 ? (
          <EmptyState>{t('metrics.units.noMatch')}</EmptyState>
        ) : (
          <>
            <UnitCatalogue
              groups={shownGroups}
              custom={custom}
              usedBy={usedBy}
            />
            <Typography variant="body2" color="text.secondary" sx={{ my: 1.5 }}>
              {t('metrics.units.count', { count: unitCount })}
            </Typography>
          </>
        )}
      </Box>
    </>
  )
}
