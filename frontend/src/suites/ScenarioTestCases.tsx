import { cx } from '@graphite/react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { Annotations } from '../annotations/Annotations'
import { useTestCaseAnnotations } from '../annotations/queries'
import type { TestCaseSummary } from '../api/client'
import { formatNumber } from '../format'
import { ErrorAlert } from '../pages/ErrorAlert'
import { Section } from '../pages/Section'
import { useRowLink } from '../pages/useRowLink'
import { useAddTestCaseTag, useRemoveTestCaseTag } from '../runs/case/mutations'
import { AssetList, Parameters } from '../runs/TestCaseParts'
import { Tags } from '../tags/Tags'
import {
  Alert,
  Button,
  Chip,
  ChipLink,
  ProgressBar,
  Stack,
  Table,
  TextLink,
} from '../ui'
import { suitesHref } from './paths'
import { casesOfVersion, type SuitesView } from './suiteModel'
import './suites.css'

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
            <span className="ui-secondary">
              {view.tag
                ? t('suites.testCases.tagged', {
                    count: shown.length,
                    total: formatNumber(all.length),
                    tag: view.tag,
                  })
                : t('suites.testCases.count', { count: all.length })}
            </span>
          )
        }
      >
        <p className="ui-text ui-secondary tr-suites-intro">
          {t('suites.testCases.description')}
        </p>
        {view.tag && (
          <div className="tr-suites-pad">
            <Chip
              tone="accent"
              outlined
              onRemove={() => navigate(suitesHref({ ...view, tag: undefined }))}
              removeLabel={t('suites.testCases.clearTag')}
            >
              {t('suites.testCases.tagFilter', { tag: view.tag })}
            </Chip>
          </div>
        )}

        <div className="tr-suites-progress">
          {query.isFetching && <ProgressBar />}
        </div>

        {query.error && (
          <ErrorAlert
            message={t('suites.testCases.error', {
              message: query.error.message,
            })}
            onRetry={() => query.refetch()}
            style={{ margin: 'var(--space-3) var(--space-4)' }}
          />
        )}

        {query.data &&
          (shown.length === 0 ? (
            <p className="ui-text ui-secondary tr-suites-note">
              {view.tag && all.length > 0
                ? t('suites.testCases.noneTagged', { tag: view.tag })
                : t('suites.testCases.empty')}
            </p>
          ) : (
            <>
              <TestCaseTable
                testCases={shown.slice(0, limit)}
                selected={selected}
                view={view}
              />
              {shown.length > limit && (
                <Stack
                  row
                  gap={4}
                  justify="space-between"
                  className="tr-suites-more"
                >
                  <p className="ui-text ui-secondary">
                    {t('suites.testCases.shown', {
                      count: limit,
                      total: formatNumber(shown.length),
                    })}
                  </p>
                  <Button
                    size="sm"
                    onClick={() => setLimit(limit + testCasePageSize)}
                  >
                    {t('suites.testCases.showMore')}
                  </Button>
                </Stack>
              )}
            </>
          ))}
      </Section>

      {selected ? (
        <TestCasePanel key={selected.id} testCase={selected} view={view} />
      ) : (
        view.testCase !== undefined &&
        query.data && (
          <Alert tone="warning">
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
    <Table aria-label={t('suites.versions.testCases')}>
      <thead>
        <tr>
          <th>{t('suites.testCases.columns.testCase')}</th>
          <th>{t('suites.testCases.columns.parameters')}</th>
          <th>{t('suites.testCases.columns.assets')}</th>
          <th>{t('suites.testCases.columns.tags')}</th>
        </tr>
      </thead>
      <tbody>
        {testCases.map((tc) => {
          const current = tc.id === selected?.id
          const href = suitesHref({
            ...view,
            version: tc.scenario_version,
            testCase: tc.id,
          })
          return (
            <tr
              key={tc.id}
              className={cx('ui-row-link', current && 'tr-suites-current')}
              onClick={rowLink(href)}
            >
              <td>
                <TextLink
                  to={href}
                  aria-label={t('suites.testCases.name', { id: tc.id })}
                  aria-current={current ? 'true' : undefined}
                  className={cx(current && 'tr-suites-current-link')}
                >
                  {tc.id}
                </TextLink>
              </td>
              <td className="ui-wrap-cell">
                <Parameters testCase={tc} />
              </td>
              <td className="ui-wrap-cell">
                <AssetList assets={tc.assets} />
              </td>
              <td className="ui-wrap-cell">
                {tc.tags.length === 0 ? (
                  t('common.none')
                ) : (
                  <Stack row wrap gap={1}>
                    {tc.tags.map((tag) => (
                      // Opens the test cases with the tag.
                      <ChipLink
                        key={tag}
                        to={suitesHref({ ...view, tag })}
                        aria-label={t('suites.testCases.tags.show', { tag })}
                      >
                        {tag}
                      </ChipLink>
                    ))}
                  </Stack>
                )}
              </td>
            </tr>
          )
        })}
      </tbody>
    </Table>
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
    <div ref={panel} className="tr-suites-case">
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
    </div>
  )
}
