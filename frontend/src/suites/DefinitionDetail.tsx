import ListAltIcon from '@mui/icons-material/ListAlt'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import Link from '@mui/material/Link'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Typography from '@mui/material/Typography'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import type { VersionedDefinition } from '../api/client'
import { formatJsonValue, formatNumber } from '../format'
import { CopyButton } from '../pages/CopyButton'
import { Facts } from '../pages/Facts'
import { Section } from '../pages/Section'
import { Time } from '../pages/Time'
import { useRowLink } from '../pages/useRowLink'
import { runListHref } from '../runs/runFilters'
import { suitesHref } from './paths'
import { useScenarioTestCases } from './queries'
import { ScenarioTestCases } from './ScenarioTestCases'
import {
  casesPerVersion,
  type DefinitionChange,
  type DefinitionEntry,
  definitionEntries,
  selectedRow,
  type SuitesView,
  type VersionRow,
  versionRows,
} from './suiteModel'

type Props = {
  definition: VersionedDefinition
  view: SuitesView
}

/**
 * One suite or scenario: its versions with what each changed, the
 * definition of the selected version and, for a scenario, the test cases
 * of that version. Definitions are uploaded by the harness and immutable
 * (FR-7): there is nothing to edit.
 */
export function DefinitionDetail({ definition, view }: Props) {
  const { t } = useTranslation()
  const isScenario = view.tab === 'scenarios'
  const rows = useMemo(() => versionRows(definition), [definition])
  const row = selectedRow(rows, view.version)
  const testCases = useScenarioTestCases(
    isScenario ? definition.name : undefined,
  )
  const caseCounts = useMemo(
    () => testCases.data && casesPerVersion(testCases.data),
    [testCases.data],
  )

  return (
    <Box sx={{ display: 'grid', gap: 2 }}>
      <Section
        title={definition.name}
        flush
        actions={
          <Button
            component={RouterLink}
            to={runListHref(
              isScenario
                ? { scenario: definition.name }
                : { suite: definition.name },
            )}
            size="small"
            variant="outlined"
            startIcon={<ListAltIcon />}
          >
            {t(`suites.kind.${view.tab}.runs`)}
          </Button>
        }
      >
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ px: 2, pb: 1.5, maxWidth: '80ch' }}
        >
          {t(`suites.kind.${view.tab}.about`)}
        </Typography>
        <VersionTable
          rows={rows}
          selected={row}
          view={view}
          name={definition.name}
          caseCounts={isScenario ? caseCounts : undefined}
          withCases={isScenario}
        />
      </Section>

      {row ? (
        <VersionDefinition row={row} />
      ) : (
        <Alert severity="warning">
          {t('suites.versions.notFound', {
            name: definition.name,
            version: view.version,
          })}
        </Alert>
      )}

      {isScenario && row && (
        <ScenarioTestCases
          version={row.version.version}
          view={view}
          query={testCases}
        />
      )}
    </Box>
  )
}

const changeKinds = ['changed', 'added', 'removed'] as const

/** The versions, newest first; a row selects its version. */
function VersionTable({
  rows,
  selected,
  view,
  name,
  caseCounts,
  withCases,
}: {
  rows: readonly VersionRow[]
  selected: VersionRow | undefined
  view: SuitesView
  name: string
  /** Test cases per version; undefined while they load. */
  caseCounts: Map<number, number> | undefined
  withCases: boolean
}) {
  const { t } = useTranslation()
  const rowLink = useRowLink()
  const latest = rows[0]?.version.version
  return (
    <TableContainer>
      <Table size="small" aria-label={t('suites.versions.title')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('suites.versions.version')}</TableCell>
            <TableCell>{t('suites.versions.uploaded')}</TableCell>
            <TableCell>{t('suites.versions.changes')}</TableCell>
            {withCases && (
              <TableCell align="right">
                {t('suites.versions.testCases')}
              </TableCell>
            )}
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => {
            const { version } = row.version
            const current = row === selected
            // Test cases belong to their version; the tag filter is kept.
            const href = suitesHref({
              tab: view.tab,
              search: view.search,
              name,
              version,
              tag: view.tag,
            })
            return (
              <TableRow
                key={row.version.id}
                hover
                selected={current}
                onClick={rowLink(href)}
                sx={{ cursor: 'pointer', verticalAlign: 'top' }}
              >
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Link
                    component={RouterLink}
                    to={href}
                    underline="hover"
                    aria-current={current ? 'true' : undefined}
                    sx={{ fontWeight: current ? 500 : undefined }}
                  >
                    {t('runs.version', { version })}
                  </Link>
                  {version === latest && (
                    <Chip
                      size="small"
                      variant="outlined"
                      label={t('suites.versions.latest')}
                      sx={{ ml: 1 }}
                    />
                  )}
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Time iso={row.version.created_at} />
                </TableCell>
                <TableCell sx={{ minWidth: 200, overflowWrap: 'anywhere' }}>
                  <Changes row={row} />
                </TableCell>
                {withCases && (
                  <TableCell align="right">
                    {caseCounts
                      ? formatNumber(caseCounts.get(version) ?? 0)
                      : ''}
                  </TableCell>
                )}
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </TableContainer>
  )
}

/** Which details a version changed, one line per kind of change. */
function Changes({ row }: { row: VersionRow }) {
  const { t } = useTranslation()
  if (!row.previous) return t('suites.versions.first')
  const kinds = changeKinds.filter((kind) => row.changes[kind].length > 0)
  if (kinds.length === 0) return t('common.none')
  return kinds.map((kind) => (
    <Box key={kind} component="span" sx={{ display: 'block' }}>
      {t(`suites.versions.${kind}`, { keys: row.changes[kind].join(', ') })}
    </Box>
  ))
}

/**
 * The definition of one version as the harness uploaded it, nested details
 * with dotted keys. What differs from the version before is marked, with
 * the value it had there.
 */
function VersionDefinition({ row }: { row: VersionRow }) {
  const { t } = useTranslation()
  const { version, previous } = row
  const entries = useMemo(
    () => definitionEntries(version.definition, previous?.definition),
    [version, previous],
  )
  const marked = entries.some((entry) => entry.change)
  return (
    <Section
      title={t('suites.definition.title', { version: version.version })}
      actions={
        <CopyButton
          text={JSON.stringify(version.definition, null, 2)}
          label={t('suites.definition.copy')}
        />
      }
    >
      {entries.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {t('suites.definition.empty')}
        </Typography>
      ) : (
        <>
          {previous && marked && (
            <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
              {t('suites.definition.compared', { version: previous.version })}
            </Typography>
          )}
          <Facts
            items={entries.map((entry) => ({
              label: entry.key,
              value: (
                <EntryValue
                  entry={entry}
                  previousVersion={previous?.version}
                />
              ),
              code: true,
            }))}
          />
        </>
      )}
    </Section>
  )
}

const changeColor: Record<DefinitionChange, 'warning' | 'success' | 'error'> = {
  changed: 'warning',
  added: 'success',
  removed: 'error',
}

/** A detail's value; a difference from the version before is said in words. */
function EntryValue({
  entry,
  previousVersion,
}: {
  entry: DefinitionEntry
  previousVersion: number | undefined
}) {
  const { t } = useTranslation()
  const value = formatJsonValue(entry.value)
  if (!entry.change) return value
  return (
    <Box
      sx={{
        pl: 1,
        borderLeft: 3,
        borderColor: `${changeColor[entry.change]}.main`,
      }}
    >
      {entry.change === 'removed' ? <s>{value}</s> : value}
      <Typography variant="caption" color="text.secondary" component="div">
        {entry.change === 'changed'
          ? t('suites.definition.was', {
              version: previousVersion,
              value: formatJsonValue(entry.previous),
            })
          : t(`suites.definition.${entry.change}`)}
      </Typography>
    </Box>
  )
}
