import { cx, Switch, TextField } from '@graphite/react'
import { FunnelX } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { CaseExecutionSummary } from '../../api/client'
import { formatDuration, formatNumber } from '../../format'
import { ErrorAlert } from '../../pages/ErrorAlert'
import { Section } from '../../pages/Section'
import { useRowLink } from '../../pages/useRowLink'
import {
  Button,
  Chip,
  Combobox,
  ProgressBar,
  Table,
  TextLink,
  Tooltip,
} from '../../ui'
import {
  type CaseFilters,
  filterCaseExecutions,
  hasActiveCaseFilters,
} from './caseFilters'
import { caseExecutionPath } from '../paths'
import { Aliased, AssetList, Parameters } from '../TestCaseParts'
import '../runs.css'

/** Rows shown at first and added by "Show more". */
export const casePageSize = 100

type Props = {
  runId: string
  query: {
    data?: CaseExecutionSummary[]
    error: Error | null
    isFetching: boolean
    refetch: () => unknown
  }
  filters: CaseFilters
  onChange: (filters: CaseFilters, options?: { replace?: boolean }) => void
}

/**
 * The case executions of a run: a scenario version with assets and
 * parameters on one client device. Filters are in the URL (FR-21); a row
 * opens the case execution.
 */
export function CaseExecutions({ runId, query, filters, onChange }: Props) {
  const { t } = useTranslation()
  const all = useMemo(() => query.data ?? [], [query.data])
  const shown = useMemo(
    () => filterCaseExecutions(all, filters),
    [all, filters],
  )
  const [limit, setLimit] = useState(casePageSize)
  const set = (patch: Partial<CaseFilters>, replace = false) =>
    onChange({ ...filters, ...patch }, { replace })

  const scenarios = useMemo(
    () => uniqueSorted(all.map((ce) => ce.test_case.scenario)),
    [all],
  )
  const devices = useMemo(
    () => uniqueSorted(all.map((ce) => ce.client_device.resolved_name)),
    [all],
  )

  return (
    <Section
      title={t('runDetail.cases.title')}
      flush
      actions={
        query.data && (
          <span className="ui-secondary">
            {hasActiveCaseFilters(filters)
              ? t('runDetail.cases.matching', {
                  count: shown.length,
                  total: formatNumber(all.length),
                })
              : t('runDetail.cases.count', { count: all.length })}
          </span>
        )
      }
    >
      <search
        aria-label={t('runDetail.cases.filters')}
        className="tr-runs-case-filters"
      >
        <NameFilter
          label={t('runDetail.cases.scenario')}
          options={scenarios}
          value={filters.scenario}
          onChange={(scenario) => set({ scenario })}
        />
        <NameFilter
          label={t('runDetail.cases.device')}
          options={devices}
          value={filters.device}
          onChange={(device) => set({ device })}
        />
        <TextField
          type="search"
          icon="search"
          className="tr-runs-filter-search"
          label={t('runDetail.cases.search')}
          placeholder={t('runDetail.cases.searchHint')}
          value={filters.search ?? ''}
          // Typing replaces the history entry instead of adding one per key.
          onChange={(event) =>
            set({ search: event.target.value || undefined }, true)
          }
        />
        <Switch
          label={t('runDetail.cases.showSuperseded')}
          checked={filters.superseded}
          onCheckedChange={(superseded) => set({ superseded })}
        />
        {hasActiveCaseFilters(filters) && (
          <Button
            variant="ghost"
            icon={FunnelX}
            onClick={() =>
              set({ scenario: undefined, device: undefined, search: undefined })
            }
          >
            {t('runDetail.cases.clear')}
          </Button>
        )}
      </search>

      <div className="tr-runs-progress">
        {query.isFetching && <ProgressBar label={t('common.loading')} />}
      </div>

      {query.error && (
        <ErrorAlert
          message={t('runDetail.cases.error', { message: query.error.message })}
          onRetry={() => query.refetch()}
          style={{ margin: 'var(--space-3) var(--space-4)' }}
        />
      )}

      {query.data &&
        (shown.length === 0 ? (
          <p className="tr-runs-note">
            {all.length === 0
              ? t('runDetail.cases.empty')
              : t('runDetail.cases.noMatch')}
          </p>
        ) : (
          <>
            <CaseTable runId={runId} caseExecutions={shown.slice(0, limit)} />
            {shown.length > limit && (
              <div className="tr-runs-list-footer tr-runs-list-footer-flush">
                <p className="ui-text ui-secondary">
                  {t('runDetail.cases.shown', {
                    count: limit,
                    total: formatNumber(shown.length),
                  })}
                </p>
                <Button
                  size="sm"
                  onClick={() => setLimit(limit + casePageSize)}
                >
                  {t('runDetail.cases.showMore')}
                </Button>
              </div>
            )}
          </>
        ))}
    </Section>
  )
}

const uniqueSorted = (values: string[]) => [...new Set(values)].sort()

type NameFilterProps = {
  label: string
  options: string[]
  value: string | undefined
  onChange: (value: string | undefined) => void
}

/** Picks one name; a name from the URL is offered even if no row has it. */
function NameFilter({ label, options, value, onChange }: NameFilterProps) {
  return (
    <Combobox<string>
      className="tr-runs-filter"
      label={label}
      options={
        value && !options.includes(value) ? [value, ...options] : options
      }
      value={value ?? null}
      onChange={(next) => onChange(next ?? undefined)}
    />
  )
}

function CaseTable({
  runId,
  caseExecutions,
}: {
  runId: string
  caseExecutions: CaseExecutionSummary[]
}) {
  const { t } = useTranslation()
  const rowLink = useRowLink()
  return (
    <Table aria-label={t('runDetail.cases.title')}>
      <thead>
        <tr>
          <th>{t('runDetail.cases.scenario')}</th>
          <th>{t('runDetail.cases.parameters')}</th>
          <th>{t('runDetail.cases.assets')}</th>
          <th>{t('runDetail.cases.device')}</th>
          <th className="g-r">{t('runDetail.cases.samples')}</th>
          <th>{t('runDetail.cases.duration')}</th>
        </tr>
      </thead>
      <tbody>
        {caseExecutions.map((ce) => {
          const href = caseExecutionPath(runId, ce.id)
          const superseded = Boolean(ce.superseded_by)
          return (
            <tr
              key={ce.id}
              className={cx('ui-row-link', superseded && 'tr-runs-dim')}
              onClick={rowLink(href)}
            >
              <td>
                <TextLink to={href}>{ce.test_case.scenario}</TextLink>{' '}
                <span className="ui-secondary">
                  {t('runs.version', {
                    version: ce.test_case.scenario_version,
                  })}
                </span>
                {superseded && (
                  <Tooltip title={t('runDetail.cases.supersededHint')}>
                    <Chip outlined style={{ marginLeft: 'var(--space-2)' }}>
                      {t('runDetail.cases.superseded')}
                    </Chip>
                  </Tooltip>
                )}
                {ce.supersedes_id && (
                  <Tooltip title={t('runDetail.cases.correctionHint')}>
                    <Chip
                      outlined
                      tone="info"
                      style={{ marginLeft: 'var(--space-2)' }}
                    >
                      {t('runDetail.cases.correction')}
                    </Chip>
                  </Tooltip>
                )}
              </td>
              <td className="ui-wrap-cell">
                <Parameters testCase={ce.test_case} />
              </td>
              <td>
                <AssetList assets={ce.test_case.assets} />
              </td>
              <td>
                <Aliased
                  shown={ce.client_device.resolved_name}
                  uploaded={ce.client_device.name}
                />
              </td>
              <td className="g-r">{formatNumber(ce.sample_count)}</td>
              <td>
                {ce.started_at && ce.ended_at
                  ? formatDuration(ce.started_at, ce.ended_at)
                  : t('common.none')}
              </td>
            </tr>
          )
        })}
      </tbody>
    </Table>
  )
}
