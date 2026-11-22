import AccountTreeIcon from '@mui/icons-material/AccountTree'
import Box from '@mui/material/Box'
import LinearProgress from '@mui/material/LinearProgress'
import List from '@mui/material/List'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemText from '@mui/material/ListItemText'
import Paper from '@mui/material/Paper'
import { useTheme } from '@mui/material/styles'
import Tab from '@mui/material/Tab'
import Tabs from '@mui/material/Tabs'
import Typography from '@mui/material/Typography'
import useMediaQuery from '@mui/material/useMediaQuery'
import { useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useSearchParams } from 'react-router'
import type { VersionedDefinition } from '../api/client'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
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

const tabId = (tab: SuitesTab) => `suites-tab-${tab}`
const panelId = (tab: SuitesTab) => `suites-panel-${tab}`

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
  const theme = useTheme()
  const stacked = useMediaQuery(theme.breakpoints.down('md'))
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

      <Box sx={{ borderBottom: 1, borderColor: 'divider', mb: 2 }}>
        <Tabs
          value={view.tab}
          // A selection belongs to its tab; the search is kept.
          onChange={(_, tab: SuitesTab) =>
            setSearchParams(suitesViewToParams({ tab, search: view.search }))
          }
          aria-label={t('suites.tabs.label')}
          variant="scrollable"
          allowScrollButtonsMobile
        >
          <Tab
            value="suites"
            label={t('suites.tabs.suites')}
            id={tabId('suites')}
            aria-controls={panelId('suites')}
          />
          <Tab
            value="scenarios"
            label={t('suites.tabs.scenarios')}
            id={tabId('scenarios')}
            aria-controls={panelId('scenarios')}
          />
        </Tabs>
      </Box>

      <Box
        role="tabpanel"
        id={panelId(view.tab)}
        aria-labelledby={tabId(view.tab)}
        sx={{
          display: 'grid',
          gap: 2,
          alignItems: 'start',
          gridTemplateColumns: {
            xs: 'minmax(0, 1fr)',
            md: 'minmax(220px, 1fr) minmax(0, 3fr)',
          },
        }}
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
        <Box ref={panel} sx={{ minWidth: 0, scrollMarginTop: 72 }}>
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
            <EmptyState icon={<AccountTreeIcon />} dashed>
              {rows.length === 0
                ? t(`suites.kind.${view.tab}.empty`)
                : t(`suites.kind.${view.tab}.selectHint`)}
            </EmptyState>
          )}
        </Box>
      </Box>
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
    <Paper
      component="nav"
      variant="outlined"
      aria-label={t(`suites.kind.${view.tab}.listLabel`)}
      sx={{ minWidth: 0, overflow: 'hidden' }}
    >
      <Box component="search" sx={{ display: 'flex', p: 1.5 }}>
        <SearchField
          label={t(`suites.kind.${view.tab}.search`)}
          value={view.search ?? ''}
          onChange={onSearch}
        />
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
          {rows.length === 0
            ? t(`suites.kind.${view.tab}.none`)
            : t(`suites.kind.${view.tab}.noMatch`)}
        </Typography>
      ) : (
        <List
          dense
          disablePadding
          sx={{ maxHeight: { md: 'calc(100vh - 330px)' }, overflowY: 'auto' }}
        >
          {shown.map((row) => {
            const current = row.name === view.name
            return (
              <ListItemButton
                key={row.id}
                component={RouterLink}
                // Selecting keeps the search; the latest version is shown.
                to={suitesHref({
                  tab: view.tab,
                  search: view.search,
                  name: row.name,
                })}
                selected={current}
                aria-current={current ? 'page' : undefined}
              >
                <ListItemText
                  primary={row.name}
                  secondary={t('suites.versions.count', {
                    count: row.versions.length,
                  })}
                  slotProps={{
                    primary: {
                      sx: { fontWeight: 500, overflowWrap: 'anywhere' },
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
          {t(`suites.kind.${view.tab}.count`, { count: shown.length })}
        </Typography>
      )}
    </Paper>
  )
}
