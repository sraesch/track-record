import { Tabs } from '@graphite/react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
import { ProgressBar } from '../ui'
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
import './metrics.css'

// Graphite's tabs name their tabs and panels after this prefix.
const idPrefix = 'metrics'

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

      <Tabs<MetricsTab>
        className="tr-metrics-tabs"
        items={[
          { value: 'metrics', label: t('metrics.tabs.metrics') },
          { value: 'units', label: t('metrics.tabs.units') },
        ]}
        value={view.tab}
        onChange={(tab) => setView({ ...view, tab })}
        aria-label={t('metrics.tabs.label')}
        idPrefix={idPrefix}
      />

      <search
        aria-label={t('metrics.searchLabel')}
        className="tr-metrics-search"
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
      </search>

      <div className="tr-metrics-progress">
        {(metrics.isFetching || units.isFetching) && <ProgressBar />}
      </div>

      <div
        role="tabpanel"
        id={`${idPrefix}-panel-${view.tab}`}
        aria-labelledby={`${idPrefix}-tab-${view.tab}`}
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
              <p className="ui-text ui-secondary small tr-metrics-count">
                {t('metrics.count', { count: shownMetrics.length })}
              </p>
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
            <p className="ui-text ui-secondary small tr-metrics-count">
              {t('metrics.units.count', { count: unitCount })}
            </p>
          </>
        )}
      </div>
    </>
  )
}
