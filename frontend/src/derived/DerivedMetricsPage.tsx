import { Plus, Sigma } from 'lucide-react'
import { useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useLocation, useSearchParams } from 'react-router'
import type { DerivedMetric } from '../api/client'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
import {
  Alert,
  breakpoints,
  LinkButton,
  ProgressBar,
  Stack,
  Surface,
  useMediaQuery,
} from '../ui'
import { DerivedMetricDetail, type SavedState } from './DerivedMetricDetail'
import { DerivedMetricEditor } from './DerivedMetricEditor'
import {
  type DerivedView,
  derivedViewToParams,
  type SetDerivedView,
  filterDerived,
  parseDerivedView,
} from './derivedModel'
import { derivedHref } from './paths'
import { useDerivedMetrics } from './queries'
import './derived.css'

/**
 * Derived metrics (FR-17): the list on the left, the selected metric or the
 * editor on the right; stacked on narrow screens, list first. Selection,
 * search and editor are in the URL (FR-21). A derived metric has one
 * definition, which the editor changes (ADR 0017).
 */
export function DerivedMetricsPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const view = useMemo(() => parseDerivedView(searchParams), [searchParams])
  const metrics = useDerivedMetrics()

  const setView: SetDerivedView = (next, options) =>
    setSearchParams(derivedViewToParams(next), options)

  const rows = useMemo(() => metrics.data ?? [], [metrics.data])
  const shown = useMemo(
    () => filterDerived(rows, view.search),
    [rows, view.search],
  )
  const selected = rows.find((row) => row.name === view.name)
  const location = useLocation()
  const deleted = (location.state as SavedState | null)?.deleted

  // Stacked on narrow screens: bring the panel into view when the
  // selection or the editor changes, not when the page opens.
  const stacked = !useMediaQuery(`(min-width:${breakpoints.md}px)`)
  const panel = useRef<HTMLDivElement>(null)
  const shownKey = `${view.name ?? ''}|${view.edit ?? ''}`
  const firstKey = useRef(shownKey)
  useEffect(() => {
    if (stacked && shownKey !== firstKey.current) {
      panel.current?.scrollIntoView?.({ block: 'start' })
    }
  }, [shownKey, stacked])

  return (
    <>
      <PageHeader
        title={t('nav.items.derivedMetrics')}
        description={t('pages.derivedMetrics.description')}
      />
      <div className="tr-derived-layout">
        <DerivedList
          view={view}
          rows={rows}
          shown={shown}
          loaded={metrics.data !== undefined}
          fetching={metrics.isFetching}
          // Typing replaces the history entry instead of adding one per
          // character.
          onSearch={(search) => setView({ ...view, search }, { replace: true })}
        />
        <div ref={panel} className="tr-derived-panel">
          {metrics.isError && !metrics.data ? (
            <ErrorAlert
              message={t('derived.error', { message: metrics.error.message })}
              onRetry={() => metrics.refetch()}
            />
          ) : !metrics.data ? null : view.edit === 'new' ? (
            <DerivedMetricEditor
              key="new"
              metrics={rows}
              view={view}
              setView={setView}
            />
          ) : view.edit === 'change' && selected ? (
            <DerivedMetricEditor
              key={selected.name}
              metric={selected}
              metrics={rows}
              view={view}
              setView={setView}
            />
          ) : selected ? (
            <DerivedMetricDetail
              metric={selected}
              metrics={rows}
              view={view}
              setView={setView}
            />
          ) : view.name ? (
            <EmptyState>
              {t('derived.notFound', { name: view.name })}
            </EmptyState>
          ) : (
            <Stack gap={4}>
              {deleted && (
                <Alert tone="success">
                  {t('derived.delete.done', { name: deleted })}
                </Alert>
              )}
              <EmptyState icon={<Sigma size={24} aria-hidden />} dashed>
                {rows.length === 0
                  ? t('derived.empty')
                  : t('derived.selectHint')}
              </EmptyState>
            </Stack>
          )}
        </div>
      </div>
    </>
  )
}

function DerivedList({
  view,
  rows,
  shown,
  loaded,
  fetching,
  onSearch,
}: {
  view: DerivedView
  rows: readonly DerivedMetric[]
  shown: readonly DerivedMetric[]
  loaded: boolean
  fetching: boolean
  onSearch: (search: string) => void
}) {
  const { t } = useTranslation()
  // Selecting a metric keeps the search and the preview case.
  const keep = {
    search: view.search,
    run: view.run,
    caseExecution: view.caseExecution,
  }
  return (
    <Surface
      as="nav"
      aria-label={t('derived.listLabel')}
      className="tr-derived-list"
    >
      <div className="tr-derived-list-head">
        <search className="tr-derived-search">
          <SearchField
            label={t('derived.search')}
            value={view.search ?? ''}
            onChange={onSearch}
          />
        </search>
        <LinkButton
          to={derivedHref({ ...keep, edit: 'new' })}
          variant="primary"
          icon={Plus}
        >
          {t('derived.new')}
        </LinkButton>
      </div>
      <div className="tr-derived-progress">{fetching && <ProgressBar />}</div>
      {loaded && shown.length === 0 ? (
        <p className="ui-text tr-derived-list-note">
          {rows.length === 0 ? t('derived.none') : t('derived.noMatch')}
        </p>
      ) : (
        <ul className="ui-list tr-derived-items">
          {shown.map((row) => {
            const current = row.name === view.name
            return (
              <li key={row.id}>
                <Link
                  to={derivedHref({ ...keep, name: row.name })}
                  aria-current={current ? 'page' : undefined}
                  className="tr-derived-item"
                >
                  <span className="tr-derived-item-head">
                    <code className="tr-derived-item-name">{row.name}</code>
                    <span className="tr-derived-item-unit">
                      {row.unit || t('metrics.unit.dimensionless')}
                    </span>
                  </span>
                  <span className="tr-derived-item-expression">
                    {row.expression}
                  </span>
                </Link>
              </li>
            )
          })}
        </ul>
      )}
      {loaded && shown.length > 0 && (
        <p className="ui-text small tr-derived-count">
          {t('derived.count', { count: shown.length })}
        </p>
      )}
    </Surface>
  )
}
