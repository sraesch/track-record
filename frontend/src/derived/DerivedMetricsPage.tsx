import AddIcon from '@mui/icons-material/Add'
import FunctionsIcon from '@mui/icons-material/Functions'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import List from '@mui/material/List'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemText from '@mui/material/ListItemText'
import Paper from '@mui/material/Paper'
import { useTheme } from '@mui/material/styles'
import Typography from '@mui/material/Typography'
import useMediaQuery from '@mui/material/useMediaQuery'
import { useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useSearchParams } from 'react-router'
import type { DerivedMetric } from '../api/client'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
import { DerivedMetricDetail } from './DerivedMetricDetail'
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

  // Stacked on narrow screens: bring the panel into view when the
  // selection or the editor changes, not when the page opens.
  const theme = useTheme()
  const stacked = useMediaQuery(theme.breakpoints.down('md'))
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
      <Box
        sx={{
          display: 'grid',
          gap: 2,
          alignItems: 'start',
          gridTemplateColumns: {
            xs: 'minmax(0, 1fr)',
            md: 'minmax(240px, 1fr) minmax(0, 2.4fr)',
          },
        }}
      >
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
        <Box ref={panel} sx={{ minWidth: 0, scrollMarginTop: 72 }}>
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
            />
          ) : view.name ? (
            <EmptyState>{t('derived.notFound', { name: view.name })}</EmptyState>
          ) : (
            <EmptyState icon={<FunctionsIcon />} dashed>
              {rows.length === 0 ? t('derived.empty') : t('derived.selectHint')}
            </EmptyState>
          )}
        </Box>
      </Box>
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
    <Paper
      component="nav"
      variant="outlined"
      aria-label={t('derived.listLabel')}
      sx={{ minWidth: 0, overflow: 'hidden' }}
    >
      <Box
        sx={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          gap: 1,
          p: 1.5,
        }}
      >
        <Box component="search" sx={{ display: 'flex', flex: '1 1 160px' }}>
          <SearchField
            label={t('derived.search')}
            value={view.search ?? ''}
            onChange={onSearch}
          />
        </Box>
        <Button
          component={RouterLink}
          to={derivedHref({ ...keep, edit: 'new' })}
          variant="contained"
          size="small"
          startIcon={<AddIcon />}
        >
          {t('derived.new')}
        </Button>
      </Box>
      <Box sx={{ height: 4 }}>
        {fetching && <LinearProgress aria-label={t('common.loading')} />}
      </Box>
      {loaded && shown.length === 0 ? (
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ px: 2, py: 2 }}
        >
          {rows.length === 0 ? t('derived.none') : t('derived.noMatch')}
        </Typography>
      ) : (
        <List
          dense
          disablePadding
          sx={{ maxHeight: { md: 'calc(100vh - 260px)' }, overflowY: 'auto' }}
        >
          {shown.map((row) => {
            const current = row.name === view.name
            return (
              <ListItemButton
                key={row.id}
                component={RouterLink}
                to={derivedHref({ ...keep, name: row.name })}
                selected={current}
                aria-current={current ? 'page' : undefined}
                sx={{ alignItems: 'flex-start' }}
              >
                <ListItemText
                  primary={
                    <Box
                      component="span"
                      sx={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        gap: 1,
                      }}
                    >
                      <Box
                        component="code"
                        sx={{
                          fontFamily: 'monospace',
                          fontWeight: 500,
                          overflowWrap: 'anywhere',
                        }}
                      >
                        {row.name}
                      </Box>
                      <Typography
                        component="span"
                        variant="caption"
                        color="text.secondary"
                        sx={{ whiteSpace: 'nowrap' }}
                      >
                        {row.unit || t('metrics.unit.dimensionless')}
                      </Typography>
                    </Box>
                  }
                  secondary={row.expression}
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
      )}
      {loaded && shown.length > 0 && (
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ px: 2, py: 1, borderTop: 1, borderColor: 'divider' }}
        >
          {t('derived.count', { count: shown.length })}
        </Typography>
      )}
    </Paper>
  )
}
