import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { PageHeader } from '../pages/PageHeader'
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

/** The run list (FR-11): newest measured start first, filtered by the URL. */
export function RunListPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const filters = useMemo(() => parseRunFilters(searchParams), [searchParams])
  const query = useMemo(() => runFiltersToQuery(filters), [filters])
  const runs = useRunList(query)

  const setFilters = (next: RunFilters) =>
    setSearchParams(runFiltersToParams(next))
  const addTag = (tag: string) => {
    if (!filters.tags.includes(tag))
      setFilters({ ...filters, tags: [...filters.tags, tag] })
  }

  const loaded = runs.data?.pages.flatMap((page) => page.runs) ?? []
  const filtered = hasActiveFilters(filters)

  return (
    <>
      <PageHeader
        title={t('nav.items.runs')}
        description={t('pages.runs.description')}
      />
      <RunFilterBar filters={filters} onChange={setFilters} />

      {/* Keeps its height so the table does not jump while refetching. */}
      <Box sx={{ height: 4, mb: 1 }}>
        {runs.isFetching && <LinearProgress aria-label={t('common.loading')} />}
      </Box>

      {/* Without rows, an error takes the whole area; with rows (a failed
          "Load more" or background refetch), the rows stay and the error
          is shown below them. */}
      {runs.isError && !runs.data ? (
        <Alert
          severity="error"
          action={
            <Button color="inherit" size="small" onClick={() => runs.refetch()}>
              {t('common.retry')}
            </Button>
          }
        >
          {t('runs.error', { message: runs.error.message })}
        </Alert>
      ) : !runs.data ? null : loaded.length === 0 ? (
        <Paper
          variant="outlined"
          sx={{ p: 4, textAlign: 'center', color: 'text.secondary' }}
        >
          <Typography>
            {filtered ? t('runs.noMatch') : t('runs.empty')}
          </Typography>
        </Paper>
      ) : (
        <>
          <RunTable runs={loaded} onTagClick={addTag} />
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              mt: 1.5,
            }}
          >
            <Typography variant="body2" color="text.secondary">
              {runs.hasNextPage
                ? t('runs.shown', { count: loaded.length })
                : t('runs.allShown', { count: loaded.length })}
            </Typography>
            {runs.hasNextPage && (
              <Button
                variant="outlined"
                size="small"
                loading={runs.isFetchingNextPage}
                disabled={runs.isPlaceholderData}
                onClick={() => runs.fetchNextPage()}
              >
                {t('runs.loadMore')}
              </Button>
            )}
          </Box>
          {runs.isError && (
            <Alert
              severity="error"
              sx={{ mt: 1.5 }}
              action={
                <Button
                  color="inherit"
                  size="small"
                  onClick={() =>
                    runs.isFetchNextPageError
                      ? runs.fetchNextPage()
                      : runs.refetch()
                  }
                >
                  {t('common.retry')}
                </Button>
              }
            >
              {t(runs.isFetchNextPageError ? 'runs.moreError' : 'runs.error', {
                message: runs.error.message,
              })}
            </Alert>
          )}
        </>
      )}
    </>
  )
}
