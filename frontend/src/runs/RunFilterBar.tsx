import ExpandLessIcon from '@mui/icons-material/ExpandLess'
import ExpandMoreIcon from '@mui/icons-material/ExpandMore'
import FilterAltOffIcon from '@mui/icons-material/FilterAltOff'
import Autocomplete from '@mui/material/Autocomplete'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Checkbox from '@mui/material/Checkbox'
import Chip from '@mui/material/Chip'
import ListItemText from '@mui/material/ListItemText'
import MenuItem from '@mui/material/MenuItem'
import TextField from '@mui/material/TextField'
import ToggleButton from '@mui/material/ToggleButton'
import ToggleButtonGroup from '@mui/material/ToggleButtonGroup'
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
import {
  cleanParam,
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

type Props = {
  filters: RunFilters
  onChange: (filters: RunFilters) => void
}

const fieldSx = { minWidth: 160, flex: '1 1 160px', maxWidth: 260 }

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
    <Box
      component="search"
      aria-label={t('runs.filters.label')}
      sx={{ display: 'flex', flexWrap: 'wrap', gap: 1.5, mb: 2 }}
    >
      <OptionFilter
        label={t('runs.filters.suite')}
        options={suites}
        value={filters.suite}
        onChange={(suite) => set({ suite })}
      />

      <TextField
        select
        size="small"
        sx={fieldSx}
        label={t('runs.filters.status')}
        value={filters.status}
        slotProps={{
          select: {
            multiple: true,
            renderValue: (selected) =>
              (selected as RunStatus[])
                .map((s) => t(`runs.status.${s}`))
                .join(', '),
          },
        }}
        onChange={(event) =>
          set({ status: event.target.value as unknown as RunStatus[] })
        }
      >
        {runStatuses.map((status) => (
          <MenuItem key={status} value={status}>
            <Checkbox
              size="small"
              checked={filters.status.includes(status)}
              sx={{ p: 0, mr: 1 }}
            />
            <ListItemText primary={t(`runs.status.${status}`)} />
          </MenuItem>
        ))}
      </TextField>

      <TextField
        select
        size="small"
        sx={fieldSx}
        label={t('runs.filters.trigger')}
        value={filters.trigger ?? ''}
        onChange={(event) =>
          set({
            trigger: (event.target.value || undefined) as
              | RunTrigger
              | undefined,
          })
        }
      >
        <MenuItem value="">
          <em>{t('runs.filters.any')}</em>
        </MenuItem>
        {runTriggers.map((trigger) => (
          <MenuItem key={trigger} value={trigger}>
            {t(`runs.trigger.${trigger}`)}
          </MenuItem>
        ))}
      </TextField>

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

      <Autocomplete
        multiple
        freeSolo
        size="small"
        sx={{ ...fieldSx, maxWidth: 360 }}
        options={[]}
        value={filters.tags}
        onChange={(_, value) =>
          set({
            tags: cleanTags(value),
          })
        }
        renderValue={(value, getItemProps) =>
          value.map((tag, index) => {
            const { key, ...itemProps } = getItemProps({ index })
            return <Chip key={key} size="small" label={tag} {...itemProps} />
          })
        }
        renderInput={(params) => (
          <TextField
            {...params}
            label={t('runs.filters.tags')}
            placeholder={
              filters.tags.length === 0 ? t('runs.filters.tagsHint') : undefined
            }
          />
        )}
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

      <ToggleButtonGroup
        size="small"
        exclusive
        aria-label={t('runs.filters.validity')}
        value={filters.validity}
        onChange={(_, value: Validity | null) => {
          if (value) set({ validity: value })
        }}
      >
        {validities.map((validity) => (
          <ToggleButton key={validity} value={validity} sx={{ px: 1.5 }}>
            {t(`runs.validity.${validity}`)}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>

      <Button
        size="small"
        aria-expanded={moreOpen}
        endIcon={moreOpen ? <ExpandLessIcon /> : <ExpandMoreIcon />}
        onClick={() => setMoreOpen(!moreOpen)}
      >
        {moreOpen
          ? t('runs.filters.fewer')
          : moreCount > 0
            ? t('runs.filters.moreActive', { count: moreCount })
            : t('runs.filters.more')}
      </Button>

      {hasActiveFilters(filters) && (
        <Button
          size="small"
          startIcon={<FilterAltOffIcon />}
          onClick={() => onChange(defaultRunFilters)}
        >
          {t('runs.filters.clear')}
        </Button>
      )}

      {moreOpen && <MoreFilters filters={filters} set={set} />}
    </Box>
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
    <Box
      sx={{ display: 'flex', flexWrap: 'wrap', gap: 1.5, flexBasis: '100%' }}
    >
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
          sx={{ alignSelf: 'center' }}
          label={t('runs.filters.environment', { id: filters.environment })}
          onDelete={() => set({ environment: undefined })}
        />
      )}
    </Box>
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
    <Autocomplete
      multiple
      freeSolo
      size="small"
      sx={{ ...fieldSx, maxWidth: 360 }}
      options={[]}
      value={value}
      inputValue={input}
      onInputChange={(_, next) => {
        setInput(next)
        setInvalid(false)
      }}
      onChange={(_, next, reason, details) => {
        const added = reason === 'createOption' ? details?.option : undefined
        if (added !== undefined && cleanParam(added) === undefined) {
          // Keep the text to fix it; MUI has just cleared the input.
          setInput(added)
          setInvalid(true)
          return
        }
        const cleaned = next.flatMap((p) => cleanParam(p) ?? [])
        onChange([...new Set(cleaned)])
      }}
      renderValue={(params, getItemProps) =>
        params.map((param, index) => {
          const { key, ...itemProps } = getItemProps({ index })
          return <Chip key={key} size="small" label={param} {...itemProps} />
        })
      }
      renderInput={(params) => (
        <TextField
          {...params}
          label={t('runs.filters.params')}
          error={invalid}
          helperText={invalid ? t('runs.filters.paramsInvalid') : undefined}
          placeholder={
            value.length === 0 ? t('runs.filters.paramsHint') : undefined
          }
        />
      )}
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
  const font = monospace ? { fontFamily: 'monospace' } : undefined
  return (
    <Autocomplete
      size="small"
      sx={{ ...fieldSx, '& input': font }}
      slotProps={{ listbox: { sx: font } }}
      options={value && !names.includes(value) ? [value, ...names] : names}
      loading={options.isPending}
      value={value ?? null}
      onChange={(_, next) => onChange(next ?? undefined)}
      renderInput={(params) => <TextField {...params} label={label} />}
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
      size="small"
      sx={{ ...fieldSx, maxWidth: 190 }}
      label={label}
      value={draft}
      onChange={(event) => {
        const day = event.target.value
        setDraft(day)
        if (day === '') {
          if (value !== undefined) onChange(undefined)
        } else if (isDay(day) && day !== value) onChange(day)
      }}
      onBlur={() => setDraft(value ?? '')}
      slotProps={{
        inputLabel: { shrink: true },
        htmlInput: { min, max },
      }}
    />
  )
}
