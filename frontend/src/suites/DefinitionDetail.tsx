import { cx } from '@graphite/react'
import { ListChecks } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { VersionedDefinition } from '../api/client'
import { formatJsonValue, formatNumber } from '../format'
import { CopyButton } from '../pages/CopyButton'
import { Facts } from '../pages/Facts'
import { Section } from '../pages/Section'
import { Time } from '../pages/Time'
import { useRowLink } from '../pages/useRowLink'
import { runListHref } from '../runs/runFilters'
import { Alert, Chip, LinkButton, Stack, Table, TextLink } from '../ui'
import { suitesHref } from './paths'
import { useScenarioTestCases } from './queries'
import { ScenarioTestCases } from './ScenarioTestCases'
import {
  casesPerVersion,
  type DefinitionEntry,
  definitionEntries,
  selectedRow,
  type SuitesView,
  type VersionRow,
  versionRows,
} from './suiteModel'
import './suites.css'

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
    <Stack gap={4}>
      <Section
        title={definition.name}
        flush
        actions={
          <LinkButton
            to={runListHref(
              isScenario
                ? { scenario: definition.name }
                : { suite: definition.name },
            )}
            size="sm"
            icon={ListChecks}
          >
            {t(`suites.kind.${view.tab}.runs`)}
          </LinkButton>
        }
      >
        <p className="ui-text ui-secondary tr-suites-intro">
          {t(`suites.kind.${view.tab}.about`)}
        </p>
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
        <Alert tone="warning">
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
    </Stack>
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
    <Table aria-label={t('suites.versions.title')}>
      <thead>
        <tr>
          <th>{t('suites.versions.version')}</th>
          <th>{t('suites.versions.uploaded')}</th>
          <th>{t('suites.versions.changes')}</th>
          {withCases && (
            <th className="g-r">{t('suites.versions.testCases')}</th>
          )}
        </tr>
      </thead>
      <tbody>
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
            <tr
              key={row.version.id}
              className={cx(
                'ui-row-link tr-suites-top',
                current && 'tr-suites-current',
              )}
              onClick={rowLink(href)}
            >
              <td>
                <span className="ui-inline-row tr-suites-gap">
                  <TextLink
                    to={href}
                    aria-current={current ? 'true' : undefined}
                    className={cx(current && 'tr-suites-current-link')}
                  >
                    {t('runs.version', { version })}
                  </TextLink>
                  {version === latest && (
                    <Chip outlined>{t('suites.versions.latest')}</Chip>
                  )}
                </span>
              </td>
              <td>
                <Time iso={row.version.created_at} />
              </td>
              <td className="ui-wrap-cell tr-suites-changes">
                <Changes row={row} />
              </td>
              {withCases && (
                <td className="g-r">
                  {caseCounts ? formatNumber(caseCounts.get(version) ?? 0) : ''}
                </td>
              )}
            </tr>
          )
        })}
      </tbody>
    </Table>
  )
}

/** Which details a version changed, one line per kind of change. */
function Changes({ row }: { row: VersionRow }) {
  const { t } = useTranslation()
  if (!row.previous) return t('suites.versions.first')
  const kinds = changeKinds.filter((kind) => row.changes[kind].length > 0)
  if (kinds.length === 0) return t('common.none')
  return kinds.map((kind) => (
    <span key={kind} className="tr-suites-block">
      {t(`suites.versions.${kind}`, { keys: row.changes[kind].join(', ') })}
    </span>
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
        <p className="ui-text ui-secondary">{t('suites.definition.empty')}</p>
      ) : (
        <>
          {previous && marked && (
            <p
              className="ui-text ui-secondary"
              style={{ marginBottom: 'var(--space-3)' }}
            >
              {t('suites.definition.compared', { version: previous.version })}
            </p>
          )}
          <Facts
            items={entries.map((entry) => ({
              label: entry.key,
              value: (
                <EntryValue entry={entry} previousVersion={previous?.version} />
              ),
              code: true,
            }))}
          />
        </>
      )}
    </Section>
  )
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
    <div className={`tr-suites-change tr-suites-change-${entry.change}`}>
      {entry.change === 'removed' ? <s>{value}</s> : value}
      <div className="small ui-secondary">
        {entry.change === 'changed'
          ? t('suites.definition.was', {
              version: previousVersion,
              value: formatJsonValue(entry.previous),
            })
          : t(`suites.definition.${entry.change}`)}
      </div>
    </div>
  )
}
