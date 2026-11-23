import { Tabs } from '@graphite/react'
import { Network } from 'lucide-react'
import { useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useSearchParams } from 'react-router'
import type { VersionedDefinition } from '../api/client'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
import { breakpoints, ProgressBar, Surface, useMediaQuery } from '../ui'
import { DefinitionDetail } from './DefinitionDetail'
import { suitesHref } from './paths'
import { useDefinitions } from './queries'
import {
  filterDefinitions,
  parseSuitesView,
  type SuitesTab,
  type SuitesView,
  suitesViewToParams,
} from './suiteModel'
import './suites.css'

const idPrefix = 'suites'

/**
 * Suites and scenarios with all their versions (FR-7): the list on the
 * left, the selected one on the right; stacked on narrow screens, list
 * first. The harness uploads them with its results, so nothing is created
 * or changed here except the tags and annotations of a test case. Tab,
 * search and selection are in the URL (FR-21).
 */
export function SuitesPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const view = useMemo(() => parseSuitesView(searchParams), [searchParams])
  const definitions = useDefinitions(view.tab)

  const rows = useMemo(() => definitions.data ?? [], [definitions.data])
  const shown = useMemo(
    () => filterDefinitions(rows, view.search),
    [rows, view.search],
  )
  const selected = rows.find((row) => row.name === view.name)

  // Stacked on narrow screens: bring the panel into view when the
  // selection changes, not when the page opens.
  const stacked = !useMediaQuery(`(min-width:${breakpoints.md}px)`)
  const panel = useRef<HTMLDivElement>(null)
  const shownKey = `${view.tab}|${view.name ?? ''}`
  const firstKey = useRef(shownKey)
  useEffect(() => {
    if (stacked && shownKey !== firstKey.current) {
      panel.current?.scrollIntoView?.({ block: 'start' })
    }
  }, [shownKey, stacked])

  return (
    <>
      <PageHeader
        title={t('nav.items.suites')}
        description={t('pages.suites.description')}
      />

      <Tabs<SuitesTab>
        className="tr-suites-tabs"
        items={[
          { value: 'suites', label: t('suites.tabs.suites') },
          { value: 'scenarios', label: t('suites.tabs.scenarios') },
        ]}
        value={view.tab}
        // A selection belongs to its tab; the search is kept.
        onChange={(tab) =>
          setSearchParams(suitesViewToParams({ tab, search: view.search }))
        }
        aria-label={t('suites.tabs.label')}
        idPrefix={idPrefix}
      />

      <div
        role="tabpanel"
        id={`${idPrefix}-panel-${view.tab}`}
        aria-labelledby={`${idPrefix}-tab-${view.tab}`}
        className="tr-suites-layout"
      >
        <DefinitionList
          view={view}
          rows={rows}
          shown={shown}
          loaded={definitions.data !== undefined}
          fetching={definitions.isFetching}
          // Typing replaces the history entry instead of adding one per
          // character.
          onSearch={(search) =>
            setSearchParams(suitesViewToParams({ ...view, search }), {
              replace: true,
            })
          }
        />
        <div ref={panel} className="tr-suites-detail">
          {definitions.isError && !definitions.data ? (
            <ErrorAlert
              message={t(`suites.kind.${view.tab}.error`, {
                message: definitions.error.message,
              })}
              onRetry={() => definitions.refetch()}
            />
          ) : !definitions.data ? null : selected ? (
            <DefinitionDetail
              // Nothing of one definition is kept for the next.
              key={`${view.tab}|${selected.name}`}
              definition={selected}
              view={view}
            />
          ) : view.name ? (
            <EmptyState>
              {t(`suites.kind.${view.tab}.notFound`, { name: view.name })}
            </EmptyState>
          ) : (
            <EmptyState icon={<Network size={24} aria-hidden />} dashed>
              {rows.length === 0
                ? t(`suites.kind.${view.tab}.empty`)
                : t(`suites.kind.${view.tab}.selectHint`)}
            </EmptyState>
          )}
        </div>
      </div>
    </>
  )
}

function DefinitionList({
  view,
  rows,
  shown,
  loaded,
  fetching,
  onSearch,
}: {
  view: SuitesView
  rows: readonly VersionedDefinition[]
  shown: readonly VersionedDefinition[]
  loaded: boolean
  fetching: boolean
  onSearch: (search: string) => void
}) {
  const { t } = useTranslation()
  return (
    <Surface
      as="nav"
      aria-label={t(`suites.kind.${view.tab}.listLabel`)}
      className="tr-suites-list"
    >
      <search className="tr-suites-search">
        <SearchField
          label={t(`suites.kind.${view.tab}.search`)}
          value={view.search ?? ''}
          onChange={onSearch}
        />
      </search>
      <div className="tr-suites-progress">{fetching && <ProgressBar />}</div>
      {loaded && shown.length === 0 ? (
        <p className="ui-text ui-secondary tr-suites-list-note">
          {rows.length === 0
            ? t(`suites.kind.${view.tab}.none`)
            : t(`suites.kind.${view.tab}.noMatch`)}
        </p>
      ) : (
        <ul className="ui-list tr-suites-items">
          {shown.map((row) => {
            const current = row.name === view.name
            return (
              <li key={row.id}>
                <RouterLink
                  className="tr-suites-item"
                  // Selecting keeps the search; the latest version is shown.
                  to={suitesHref({
                    tab: view.tab,
                    search: view.search,
                    name: row.name,
                  })}
                  aria-current={current ? 'page' : undefined}
                >
                  <span className="tr-suites-item-name">{row.name}</span>
                  <span className="small ui-secondary">
                    {t('suites.versions.count', {
                      count: row.versions.length,
                    })}
                  </span>
                </RouterLink>
              </li>
            )
          })}
        </ul>
      )}
      {loaded && shown.length > 0 && (
        <p className="ui-text small ui-secondary tr-suites-list-count">
          {t(`suites.kind.${view.tab}.count`, { count: shown.length })}
        </p>
      )}
    </Surface>
  )
}
