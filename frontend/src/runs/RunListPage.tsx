import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { Button, ProgressBar } from '../ui'
import { useRunList } from './queries'
import { RunFilterBar } from './RunFilterBar'
import {
  hasActiveFilters,
  parseRunFilters,
  type RunFilters,
  runFiltersToParams,
  runFiltersToQuery,
} from './runFilters'
import { RunTable } from './RunTable'
import './runs.css'

/** The run list (FR-11): newest measured start first, filtered by the URL. */
export function RunListPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const filters = useMemo(() => parseRunFilters(searchParams), [searchParams])
  const query = useMemo(() => runFiltersToQuery(filters), [filters])
  const runs = useRunList(query)

  // Rendered at once, not in a transition: until the page shows the new
  // filters, the filter bar would build the next change from the old ones
  // (a tag typed and entered right after another would replace it).
  const setFilters = (next: RunFilters) =>
    setSearchParams(runFiltersToParams(next), { flushSync: true })
  const addTag = (tag: string) => {
    if (!filters.tags.includes(tag))
      setFilters({ ...filters, tags: [...filters.tags, tag] })
  }

  const loaded = runs.data?.pages.flatMap((page) => page.runs) ?? []
  const filtered = hasActiveFilters(filters)

  // A failed "Load more" retries that page; anything else, the whole list.
  const errorAlert = runs.isError && (
    <ErrorAlert
      message={t(runs.isFetchNextPageError ? 'runs.moreError' : 'runs.error', {
        message: runs.error.message,
      })}
      onRetry={() =>
        runs.isFetchNextPageError ? runs.fetchNextPage() : runs.refetch()
      }
    />
  )

  return (
    <>
      <PageHeader
        title={t('nav.items.runs')}
        description={t('pages.runs.description')}
      />
      <RunFilterBar filters={filters} onChange={setFilters} />

      {/* Keeps its height so the table does not jump while refetching. */}
      <div className="tr-runs-progress">
        {runs.isFetching && <ProgressBar label={t('common.loading')} />}
      </div>

      {/* Without rows, an error takes the whole area; with rows (a failed
          "Load more" or background refetch), the rows stay and the error
          is shown below them. */}
      {!runs.data ? (
        errorAlert
      ) : loaded.length === 0 ? (
        <EmptyState>
          {filtered ? t('runs.noMatch') : t('runs.empty')}
        </EmptyState>
      ) : (
        <>
          <RunTable runs={loaded} onTagClick={addTag} />
          <div className="tr-runs-list-footer">
            <p className="ui-text ui-secondary">
              {runs.hasNextPage
                ? t('runs.shown', { count: loaded.length })
                : t('runs.allShown', { count: loaded.length })}
            </p>
            {runs.hasNextPage && (
              <Button
                size="sm"
                loading={runs.isFetchingNextPage}
                disabled={runs.isPlaceholderData}
                onClick={() => runs.fetchNextPage()}
              >
                {t('runs.loadMore')}
              </Button>
            )}
          </div>
          {errorAlert}
        </>
      )}
    </>
  )
}
