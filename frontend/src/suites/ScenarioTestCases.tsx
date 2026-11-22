import CancelIcon from '@mui/icons-material/Cancel'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import LinearProgress from '@mui/material/LinearProgress'
import Link from '@mui/material/Link'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Typography from '@mui/material/Typography'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink, useNavigate } from 'react-router'
import { Annotations } from '../annotations/Annotations'
import { useTestCaseAnnotations } from '../annotations/queries'
import type { TestCaseSummary } from '../api/client'
import { formatNumber } from '../format'
import { ErrorAlert } from '../pages/ErrorAlert'
import { Section } from '../pages/Section'
import { useRowLink } from '../pages/useRowLink'
import {
  useAddTestCaseTag,
  useRemoveTestCaseTag,
} from '../runs/case/mutations'
import { AssetList, Parameters } from '../runs/TestCaseParts'
import { Tags } from '../tags/Tags'
import { suitesHref } from './paths'
import { casesOfVersion, type SuitesView } from './suiteModel'

/** Rows shown at first and added by "Show more". */
export const testCasePageSize = 100

type Props = {
  /** The selected scenario version. */
  version: number
  view: SuitesView
  /** The test cases of all versions of the scenario. */
  query: {
    data?: TestCaseSummary[]
    error: Error | null
    isFetching: boolean
    refetch: () => unknown
  }
}

/**
 * The test cases of a scenario version: the scenario with assets and
 * parameter values, after aliases (FR-14). A row selects a test case and
 * shows its tags and annotations, which every execution of it shares.
 */
export function ScenarioTestCases({ version, view, query }: Props) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const all = useMemo(
    () => casesOfVersion(query.data ?? [], version),
    [query.data, version],
  )
  const shown = useMemo(
    () => casesOfVersion(all, version, view.tag),
    [all, version, view.tag],
  )
  const [limit, setLimit] = useState(testCasePageSize)
  // Found among all test cases of the version: the tag filter does not
  // close the test case whose tag was just removed.
  const selected = all.find((tc) => tc.id === view.testCase)

  return (
    <>
      <Section
        title={t('suites.testCases.title', { version })}
        flush
        actions={
          query.data && (
            <Typography variant="body2" color="text.secondary">
              {view.tag
                ? t('suites.testCases.tagged', {
                    count: shown.length,
                    total: formatNumber(all.length),
                    tag: view.tag,
                  })
                : t('suites.testCases.count', { count: all.length })}
            </Typography>
          )
        }
      >
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ px: 2, pb: 1.5, maxWidth: '80ch' }}
        >
          {t('suites.testCases.description')}
        </Typography>
        {view.tag && (
          <Box sx={{ px: 2, pb: 1.5 }}>
            <Chip
              size="small"
              color="primary"
              variant="outlined"
              label={t('suites.testCases.tagFilter', { tag: view.tag })}
              onDelete={() => navigate(suitesHref({ ...view, tag: undefined }))}
              deleteIcon={
                <CancelIcon titleAccess={t('suites.testCases.clearTag')} />
              }
            />
          </Box>
        )}

        <Box sx={{ height: 4 }}>
          {query.isFetching && (
            <LinearProgress aria-label={t('common.loading')} />
          )}
        </Box>

        {query.error && (
          <ErrorAlert
            message={t('suites.testCases.error', {
              message: query.error.message,
            })}
            onRetry={() => query.refetch()}
            sx={{ mx: 2, my: 1.5 }}
          />
        )}

        {query.data &&
          (shown.length === 0 ? (
            <Typography variant="body2" color="text.secondary" sx={{ p: 2 }}>
              {view.tag && all.length > 0
                ? t('suites.testCases.noneTagged', { tag: view.tag })
                : t('suites.testCases.empty')}
            </Typography>
          ) : (
            <>
              <TestCaseTable
                testCases={shown.slice(0, limit)}
                selected={selected}
                view={view}
              />
              {shown.length > limit && (
                <Box
                  sx={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    gap: 2,
                    px: 2,
                    py: 1.5,
                  }}
                >
                  <Typography variant="body2" color="text.secondary">
                    {t('suites.testCases.shown', {
                      count: limit,
                      total: formatNumber(shown.length),
                    })}
                  </Typography>
                  <Button
                    variant="outlined"
                    size="small"
                    onClick={() => setLimit(limit + testCasePageSize)}
                  >
                    {t('suites.testCases.showMore')}
                  </Button>
                </Box>
              )}
            </>
          ))}
      </Section>

      {selected ? (
        <TestCasePanel key={selected.id} testCase={selected} view={view} />
      ) : (
        view.testCase !== undefined &&
        query.data && (
          <Alert severity="warning">
            {t('suites.testCases.notFound', { version, id: view.testCase })}
          </Alert>
        )
      )}
    </>
  )
}

function TestCaseTable({
  testCases,
  selected,
  view,
}: {
  testCases: readonly TestCaseSummary[]
  selected: TestCaseSummary | undefined
  view: SuitesView
}) {
  const { t } = useTranslation()
  const rowLink = useRowLink()
  return (
    <TableContainer>
      <Table size="small" aria-label={t('suites.versions.testCases')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('suites.testCases.columns.testCase')}</TableCell>
            <TableCell>{t('suites.testCases.columns.parameters')}</TableCell>
            <TableCell>{t('suites.testCases.columns.assets')}</TableCell>
            <TableCell>{t('suites.testCases.columns.tags')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {testCases.map((tc) => {
            const current = tc.id === selected?.id
            const href = suitesHref({
              ...view,
              version: tc.scenario_version,
              testCase: tc.id,
            })
            return (
              <TableRow
                key={tc.id}
                hover
                selected={current}
                onClick={rowLink(href)}
                sx={{ cursor: 'pointer' }}
              >
                <TableCell>
                  <Link
                    component={RouterLink}
                    to={href}
                    underline="hover"
                    aria-label={t('suites.testCases.name', { id: tc.id })}
                    aria-current={current ? 'true' : undefined}
                    sx={{ fontWeight: current ? 500 : undefined }}
                  >
                    {tc.id}
                  </Link>
                </TableCell>
                <TableCell>
                  <Parameters testCase={tc} />
                </TableCell>
                <TableCell>
                  <AssetList assets={tc.assets} />
                </TableCell>
                <TableCell>
                  {tc.tags.length === 0 ? (
                    t('common.none')
                  ) : (
                    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
                      {tc.tags.map((tag) => (
                        <Chip
                          key={tag}
                          size="small"
                          label={tag}
                          component={RouterLink}
                          to={suitesHref({ ...view, tag })}
                          clickable
                          aria-label={t('suites.testCases.tags.show', { tag })}
                        />
                      ))}
                    </Box>
                  )}
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </TableContainer>
  )
}

/**
 * Tags and annotations of the selected test case. They are on the case, so
 * every execution of it, in every run, shows them. A tag shows the test
 * cases of the scenario version that have it.
 */
function TestCasePanel({
  testCase,
  view,
}: {
  testCase: TestCaseSummary
  view: SuitesView
}) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { id } = testCase
  const annotations = useTestCaseAnnotations(id)

  // It opens below the table, which can be long: bring it into view.
  const panel = useRef<HTMLDivElement>(null)
  useEffect(() => {
    panel.current?.scrollIntoView?.({ block: 'nearest' })
  }, [])

  return (
    <Box
      ref={panel}
      sx={{ display: 'grid', gap: 2, minWidth: 0, scrollMarginBlock: 72 }}
    >
      <Tags
        title={t('suites.testCases.tags.title', { id })}
        description={t('suites.testCases.tags.description')}
        tags={testCase.tags}
        add={useAddTestCaseTag(id)}
        remove={useRemoveTestCaseTag(id)}
        open={{
          label: (tag) => t('suites.testCases.tags.show', { tag }),
          onOpen: (tag) => navigate(suitesHref({ ...view, tag })),
        }}
      />
      <Annotations
        title={t('suites.testCases.annotations.title', { id })}
        description={t('suites.testCases.annotations.description')}
        annotations={annotations.data}
        target={{ test_case_id: id }}
        loadError={
          annotations.isError
            ? {
                message: annotations.error.message,
                onRetry: () => annotations.refetch(),
              }
            : undefined
        }
      />
    </Box>
  )
}
