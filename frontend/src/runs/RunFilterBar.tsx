import { SegmentedControl, TextField } from '@graphite/react'
import { ChevronDown, ChevronUp, FunnelX } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { RunStatus, RunTrigger, Validity } from '../api/client'
import {
  useAssetNames,
  useBranches,
  useBuildConfigs,
  useClientDeviceNames,
  useCommits,
  useMetricNames,
  useScenarioNames,
  useSuiteNames,
  useSutTags,
} from '../api/lookups'
import { Button, Chip, Combobox, MultiSelect, Select, TagField } from '../ui'
import {
  cleanParam,
  cleanParams,
  cleanTags,
  countMoreFilters,
  defaultRunFilters,
  hasActiveFilters,
  isDay,
  type RunFilters,
  runStatuses,
  runTriggers,
  validities,
} from './runFilters'
import './runs.css'

type Props = {
  filters: RunFilters
  onChange: (filters: RunFilters) => void
}

/**
 * Filters of the run list (FR-11); every change goes to the URL. The more
 * specific ones are behind "More filters", which starts open when one of
 * them is set (e.g. from a shared link).
 */
export function RunFilterBar({ filters, onChange }: Props) {
  const { t } = useTranslation()
  const suites = useSuiteNames()
  const devices = useClientDeviceNames()
  const branches = useBranches()
  const moreCount = countMoreFilters(filters)
  const [moreOpen, setMoreOpen] = useState(moreCount > 0)
  const set = (patch: Partial<RunFilters>) => onChange({ ...filters, ...patch })

  return (
    <search aria-label={t('runs.filters.label')} className="tr-runs-filters">
      <OptionFilter
        label={t('runs.filters.suite')}
        options={suites}
        value={filters.suite}
        onChange={(suite) => set({ suite })}
      />

      <MultiSelect<RunStatus>
        className="tr-runs-filter"
        label={t('runs.filters.status')}
        options={runStatuses.map((status) => ({
          value: status,
          label: t(`runs.status.${status}`),
        }))}
        value={filters.status}
        onChange={(status) => set({ status })}
      />

      <Select<RunTrigger | ''>
        className="tr-runs-filter"
        label={t('runs.filters.trigger')}
        options={[
          { value: '', label: t('runs.filters.any') },
          ...runTriggers.map((trigger) => ({
            value: trigger,
            label: t(`runs.trigger.${trigger}`),
          })),
        ]}
        value={filters.trigger ?? ''}
        onChange={(trigger) => set({ trigger: trigger || undefined })}
      />

      <OptionFilter
        label={t('runs.filters.branch')}
        options={branches}
        value={filters.branch}
        onChange={(branch) => set({ branch })}
      />

      <OptionFilter
        label={t('runs.filters.device')}
        options={devices}
        value={filters.device}
        onChange={(device) => set({ device })}
      />

      <TagField
        className="tr-runs-filter tr-runs-filter-wide"
        label={t('runs.filters.tags')}
        values={filters.tags}
        onChange={(tags) => set({ tags: cleanTags(tags) })}
        placeholder={
          filters.tags.length === 0 ? t('runs.filters.tagsHint') : undefined
        }
      />

      <DayField
        label={t('runs.filters.from')}
        value={filters.from}
        max={filters.to}
        onChange={(from) => set({ from })}
      />
      <DayField
        label={t('runs.filters.to')}
        value={filters.to}
        min={filters.from}
        onChange={(to) => set({ to })}
      />

      <SegmentedControl<Validity>
        aria-label={t('runs.filters.validity')}
        items={validities.map((validity) => ({
          value: validity,
          label: t(`runs.validity.${validity}`),
        }))}
        value={filters.validity}
        onChange={(validity) => set({ validity })}
      />

      <Button
        variant="ghost"
        aria-expanded={moreOpen}
        onClick={() => setMoreOpen(!moreOpen)}
      >
        {moreOpen
          ? t('runs.filters.fewer')
          : moreCount > 0
            ? t('runs.filters.moreActive', { count: moreCount })
            : t('runs.filters.more')}
        {moreOpen ? (
          <ChevronUp size={14} aria-hidden />
        ) : (
          <ChevronDown size={14} aria-hidden />
        )}
      </Button>

      {hasActiveFilters(filters) && (
        <Button
          variant="ghost"
          icon={FunnelX}
          onClick={() => onChange(defaultRunFilters)}
        >
          {t('runs.filters.clear')}
        </Button>
      )}

      {moreOpen && <MoreFilters filters={filters} set={set} />}
    </search>
  )
}

/**
 * The more specific filters: SUT version details, shared environment, and
 * what the case executions of a run contain. Their options load only when
 * they are shown.
 */
function MoreFilters({
  filters,
  set,
}: {
  filters: RunFilters
  set: (patch: Partial<RunFilters>) => void
}) {
  const { t } = useTranslation()
  const commits = useCommits()
  const sutTags = useSutTags()
  const buildConfigs = useBuildConfigs()
  const scenarios = useScenarioNames()
  const assets = useAssetNames()
  const metrics = useMetricNames()

  return (
    // A row of its own below the common filters.
    <div className="tr-runs-filters-more">
      <OptionFilter
        label={t('runs.filters.commit')}
        options={commits}
        value={filters.commit}
        onChange={(commit) => set({ commit })}
        monospace
      />
      <OptionFilter
        label={t('runs.filters.sutTag')}
        options={sutTags}
        value={filters.sutTag}
        onChange={(sutTag) => set({ sutTag })}
      />
      <OptionFilter
        label={t('runs.filters.buildConfig')}
        options={buildConfigs}
        value={filters.buildConfig}
        onChange={(buildConfig) => set({ buildConfig })}
      />
      <OptionFilter
        label={t('runs.filters.scenario')}
        options={scenarios}
        value={filters.scenario}
        onChange={(scenario) => set({ scenario })}
      />
      <OptionFilter
        label={t('runs.filters.asset')}
        options={assets}
        value={filters.asset}
        onChange={(asset) => set({ asset })}
      />
      <OptionFilter
        label={t('runs.filters.metric')}
        options={metrics}
        value={filters.metric}
        onChange={(metric) => set({ metric })}
      />
      <ParamFilter
        value={filters.params}
        onChange={(params) => set({ params })}
      />
      {filters.environment !== undefined && (
        <Chip
          className="tr-runs-filter-chip"
          onRemove={() => set({ environment: undefined })}
          removeLabel={t('runs.filters.removeEnvironment')}
        >
          {t('runs.filters.environment', { id: filters.environment })}
        </Chip>
      )}
    </div>
  )
}

type ParamFilterProps = {
  value: string[]
  onChange: (params: string[]) => void
}

/**
 * Parameter filters as `name=value` chips. The value is read as JSON by the
 * API, so `size=10` matches the number 10 and `size="10"` the string. Text
 * that is not `name=value` stays in the input with a hint.
 */
function ParamFilter({ value, onChange }: ParamFilterProps) {
  const { t } = useTranslation()
  const [input, setInput] = useState('')
  const [invalid, setInvalid] = useState(false)
  return (
    <TagField
      className="tr-runs-filter tr-runs-filter-wide"
      label={t('runs.filters.params')}
      values={value}
      inputValue={input}
      onInputChange={(next) => {
        setInput(next)
        setInvalid(false)
      }}
      onAdd={(text) => {
        if (cleanParam(text) !== undefined) return true
        // Keep the text to fix it.
        setInvalid(true)
        return false
      }}
      onChange={(next) => onChange(cleanParams(next))}
      invalid={invalid}
      help={invalid ? t('runs.filters.paramsInvalid') : undefined}
      placeholder={
        value.length === 0 ? t('runs.filters.paramsHint') : undefined
      }
    />
  )
}

type OptionFilterProps = {
  label: string
  /** The query of the names to choose from. */
  options: { data?: string[]; isPending: boolean }
  value: string | undefined
  onChange: (value: string | undefined) => void
  /** For values like commit hashes. */
  monospace?: boolean
}

/**
 * Picks one name from a list. The current value is offered even when it is
 * not in the list (a shared link, a name from before an alias, options
 * still loading).
 */
function OptionFilter({
  label,
  options,
  value,
  onChange,
  monospace,
}: OptionFilterProps) {
  const names = options.data ?? []
  return (
    <Combobox<string>
      className="tr-runs-filter"
      label={label}
      mono={monospace}
      options={value && !names.includes(value) ? [value, ...names] : names}
      loading={options.isPending}
      value={value ?? null}
      onChange={(next) => onChange(next ?? undefined)}
    />
  )
}

type DayFieldProps = {
  label: string
  value: string | undefined
  min?: string
  max?: string
  onChange: (day: string | undefined) => void
}

/**
 * A date filter. While a year is typed, the input passes through days like
 * 0002-11-01; only complete, plausible days go to the URL, so typing makes
 * neither history entries nor requests for each digit.
 */
function DayField({ label, value, min, max, onChange }: DayFieldProps) {
  const [draft, setDraft] = useState(value ?? '')
  const [shown, setShown] = useState(value)
  if (value !== shown) {
    // The URL changed (back button, "Clear filters"): show its day.
    setShown(value)
    setDraft(value ?? '')
  }
  return (
    <TextField
      type="date"
      className="tr-runs-filter tr-runs-filter-day"
      label={label}
      value={draft}
      min={min}
      max={max}
      onChange={(event) => {
        const day = event.target.value
        setDraft(day)
        if (day === '') {
          if (value !== undefined) onChange(undefined)
        } else if (isDay(day) && day !== value) onChange(day)
      }}
      onBlur={() => setDraft(value ?? '')}
    />
  )
}
