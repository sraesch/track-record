import FilterAltOffIcon from '@mui/icons-material/FilterAltOff'
import SearchIcon from '@mui/icons-material/Search'
import Autocomplete from '@mui/material/Autocomplete'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import FormControlLabel from '@mui/material/FormControlLabel'
import InputAdornment from '@mui/material/InputAdornment'
import LinearProgress from '@mui/material/LinearProgress'
import Link from '@mui/material/Link'
import Switch from '@mui/material/Switch'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import TextField from '@mui/material/TextField'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import type { CaseExecutionSummary } from '../../api/client'
import { formatDuration, formatJsonValue, formatNumber } from '../../format'
import { ErrorAlert } from '../../pages/ErrorAlert'
import { Section } from '../../pages/Section'
import { useRowLink } from '../../pages/useRowLink'
import {
  type CaseFilters,
  filterCaseExecutions,
  hasActiveCaseFilters,
} from './caseFilters'
import { jsonObject } from './json'
import { caseExecutionPath } from '../paths'

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
          <Typography variant="body2" color="text.secondary">
            {hasActiveCaseFilters(filters)
              ? t('runDetail.cases.matching', {
                  count: shown.length,
                  total: formatNumber(all.length),
                })
              : t('runDetail.cases.count', { count: all.length })}
          </Typography>
        )
      }
    >
      <Box
        component="search"
        aria-label={t('runDetail.cases.filters')}
        sx={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          gap: 1.5,
          px: 2,
          pb: 1.5,
        }}
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
          size="small"
          type="search"
          sx={{ flex: '1 1 200px', maxWidth: 320 }}
          label={t('runDetail.cases.search')}
          placeholder={t('runDetail.cases.searchHint')}
          value={filters.search ?? ''}
          // Typing replaces the history entry instead of adding one per key.
          onChange={(event) =>
            set({ search: event.target.value || undefined }, true)
          }
          slotProps={{
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <SearchIcon fontSize="small" />
                </InputAdornment>
              ),
            },
          }}
        />
        <FormControlLabel
          control={
            <Switch
              size="small"
              checked={filters.superseded}
              onChange={(event) => set({ superseded: event.target.checked })}
            />
          }
          label={t('runDetail.cases.showSuperseded')}
        />
        {hasActiveCaseFilters(filters) && (
          <Button
            size="small"
            startIcon={<FilterAltOffIcon />}
            onClick={() =>
              set({ scenario: undefined, device: undefined, search: undefined })
            }
          >
            {t('runDetail.cases.clear')}
          </Button>
        )}
      </Box>

      <Box sx={{ height: 4 }}>
        {query.isFetching && (
          <LinearProgress aria-label={t('common.loading')} />
        )}
      </Box>

      {query.error && (
        <ErrorAlert
          message={t('runDetail.cases.error', { message: query.error.message })}
          onRetry={() => query.refetch()}
          sx={{ mx: 2, my: 1.5 }}
        />
      )}

      {query.data &&
        (shown.length === 0 ? (
          <Typography variant="body2" color="text.secondary" sx={{ p: 2 }}>
            {all.length === 0
              ? t('runDetail.cases.empty')
              : t('runDetail.cases.noMatch')}
          </Typography>
        ) : (
          <>
            <CaseTable runId={runId} caseExecutions={shown.slice(0, limit)} />
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
                  {t('runDetail.cases.shown', {
                    count: limit,
                    total: formatNumber(shown.length),
                  })}
                </Typography>
                <Button
                  variant="outlined"
                  size="small"
                  onClick={() => setLimit(limit + casePageSize)}
                >
                  {t('runDetail.cases.showMore')}
                </Button>
              </Box>
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
    <Autocomplete
      size="small"
      sx={{ flex: '1 1 180px', maxWidth: 260 }}
      options={value && !options.includes(value) ? [value, ...options] : options}
      value={value ?? null}
      onChange={(_, next) => onChange(next ?? undefined)}
      renderInput={(params) => <TextField {...params} label={label} />}
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
    <TableContainer>
      <Table size="small" aria-label={t('runDetail.cases.title')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('runDetail.cases.scenario')}</TableCell>
            <TableCell>{t('runDetail.cases.parameters')}</TableCell>
            <TableCell>{t('runDetail.cases.assets')}</TableCell>
            <TableCell>{t('runDetail.cases.device')}</TableCell>
            <TableCell align="right">{t('runDetail.cases.samples')}</TableCell>
            <TableCell>{t('runDetail.cases.duration')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {caseExecutions.map((ce) => {
            const href = caseExecutionPath(runId, ce.id)
            const superseded = Boolean(ce.superseded_by)
            return (
              <TableRow
                key={ce.id}
                hover
                onClick={rowLink(href)}
                sx={{ cursor: 'pointer', opacity: superseded ? 0.6 : 1 }}
              >
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Link component={RouterLink} to={href} underline="hover">
                    {ce.test_case.scenario}
                  </Link>{' '}
                  <Typography component="span" variant="body2" color="text.secondary">
                    {t('runs.version', { version: ce.test_case.scenario_version })}
                  </Typography>
                  {superseded && (
                    <Tooltip
                      describeChild
                      title={t('runDetail.cases.supersededHint')}
                    >
                      <Chip
                        size="small"
                        variant="outlined"
                        label={t('runDetail.cases.superseded')}
                        sx={{ ml: 1 }}
                      />
                    </Tooltip>
                  )}
                  {ce.supersedes_id && (
                    <Tooltip
                      describeChild
                      title={t('runDetail.cases.correctionHint')}
                    >
                      <Chip
                        size="small"
                        variant="outlined"
                        color="info"
                        label={t('runDetail.cases.correction')}
                        sx={{ ml: 1 }}
                      />
                    </Tooltip>
                  )}
                </TableCell>
                <TableCell>
                  <Parameters caseExecution={ce} />
                </TableCell>
                <TableCell>
                  {ce.test_case.assets.length === 0
                    ? t('common.none')
                    : ce.test_case.assets.map((asset) => (
                        <Tooltip
                          describeChild
                          key={asset.id}
                          title={asset.content_hash ?? ''}
                        >
                          <Box
                            component="span"
                            sx={{ display: 'block', whiteSpace: 'nowrap' }}
                          >
                            {asset.version
                              ? t('runDetail.cases.assetVersion', {
                                  name: asset.name,
                                  version: asset.version,
                                })
                              : asset.name}
                          </Box>
                        </Tooltip>
                      ))}
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Aliased
                    shown={ce.client_device.resolved_name}
                    uploaded={ce.client_device.name}
                  />
                </TableCell>
                <TableCell align="right">{formatNumber(ce.sample_count)}</TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  {ce.started_at && ce.ended_at
                    ? formatDuration(ce.started_at, ce.ended_at)
                    : t('common.none')}
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </TableContainer>
  )
}

/** Parameters after aliases (FR-14), with their units (FR-13a). */
function Parameters({ caseExecution }: { caseExecution: CaseExecutionSummary }) {
  const { t } = useTranslation()
  const tc = caseExecution.test_case
  const resolved = jsonObject(tc.resolved_parameters)
  const uploaded = jsonObject(tc.parameters)
  const units = jsonObject(tc.parameter_units)
  const names = Object.keys(resolved).sort()
  if (names.length === 0) return t('common.none')

  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
      {names.map((name) => {
        const unit = units[name]
        const value = formatJsonValue(resolved[name])
        const label =
          typeof unit === 'string' && unit
            ? t('runDetail.cases.parameterWithUnit', { name, value, unit })
            : t('runDetail.cases.parameter', { name, value })
        const chip = <Chip size="small" variant="outlined" label={label} />
        // JSON comparison: an alias may also change the type (FR-14).
        const aliased =
          name in uploaded &&
          JSON.stringify(uploaded[name]) !== JSON.stringify(resolved[name])
        return aliased ? (
          <Tooltip describeChild
            key={name}
            title={t('runDetail.cases.uploadedAs', {
              value: formatJsonValue(uploaded[name]),
            })}
          >
            {chip}
          </Tooltip>
        ) : (
          <Box component="span" key={name}>
            {chip}
          </Box>
        )
      })}
    </Box>
  )
}

/** A name after aliases; the uploaded one in the tooltip if it differs. */
function Aliased({ shown, uploaded }: { shown: string; uploaded: string }) {
  const { t } = useTranslation()
  if (shown === uploaded) return shown
  return (
    <Tooltip describeChild title={t('runDetail.cases.uploadedAs', { value: uploaded })}>
      <Box
        component="span"
        sx={{ textDecoration: 'underline dotted', textUnderlineOffset: 3 }}
      >
        {shown}
      </Box>
    </Tooltip>
  )
}
