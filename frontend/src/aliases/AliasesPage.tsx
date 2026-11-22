import AddIcon from '@mui/icons-material/Add'
import ArrowForwardIcon from '@mui/icons-material/ArrowForward'
import DeleteOutlinedIcon from '@mui/icons-material/DeleteOutlined'
import EditOutlinedIcon from '@mui/icons-material/EditOutlined'
import RestoreIcon from '@mui/icons-material/Restore'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import FormControlLabel from '@mui/material/FormControlLabel'
import IconButton from '@mui/material/IconButton'
import LinearProgress from '@mui/material/LinearProgress'
import MenuItem from '@mui/material/MenuItem'
import Paper from '@mui/material/Paper'
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
import { useSearchParams } from 'react-router'
import { formatDateTime, formatTimestamp } from '../format'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { visuallyHidden } from '../pages/visuallyHidden'
import { AliasDialog, type AliasDialogMode } from './AliasDialog'
import {
  type AliasFilters,
  aliasFiltersToParams,
  type AliasRow,
  deviceDimension,
  filterRows,
  parameterDimension,
  parseAliasFilters,
  parseDimension,
  resolve,
  sameValue,
} from './aliasModel'
import { isKnown, useAliases, useKnownValues, useSetAlias } from './queries'
import { useValueLabel } from './useValueLabel'
import { ValueLabel } from './ValueLabel'

/**
 * Aliases (FR-14): rename or merge client device names and parameter
 * values. They apply to every view, the REST API and MCP. Each change is a
 * new version (FR-7); a removed alias can be set again. Filters are in the
 * URL (FR-21).
 */
export function AliasesPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const filters = useMemo(() => parseAliasFilters(searchParams), [searchParams])
  const aliases = useAliases()
  const known = useKnownValues()
  const rows = useMemo(() => aliases.data ?? [], [aliases.data])
  const shown = useMemo(() => filterRows(rows, filters), [rows, filters])

  const [dialog, setDialog] = useState<{
    open: boolean
    mode: AliasDialogMode
    key: number
  }>({ open: false, mode: { kind: 'add' }, key: 0 })
  const openDialog = (mode: AliasDialogMode) =>
    setDialog((d) => ({ open: true, mode, key: d.key + 1 }))
  const [removing, setRemoving] = useState<AliasRow>()

  // Typing in the search replaces the history entry instead of adding one
  // per character.
  const setFilters = (next: AliasFilters, replace = false) =>
    setSearchParams(aliasFiltersToParams(next), { replace })

  const dimensions = useMemo(() => {
    const names = new Set(known?.parameterNames ?? [])
    for (const row of rows) {
      const dimension = parseDimension(row.dimension)
      if (dimension?.kind === 'parameter') names.add(dimension.name)
    }
    if (filters.dimension) {
      const dimension = parseDimension(filters.dimension)
      if (dimension?.kind === 'parameter') names.add(dimension.name)
    }
    return [...names].sort()
  }, [known, rows, filters.dimension])

  return (
    <>
      <PageHeader
        title={t('nav.items.aliases')}
        description={t('pages.aliases.description')}
        actions={
          <Button
            variant="contained"
            startIcon={<AddIcon />}
            onClick={() => openDialog({ kind: 'add' })}
          >
            {t('aliases.add')}
          </Button>
        }
      />

      <Box
        component="search"
        aria-label={t('aliases.filters.label')}
        sx={{
          display: 'flex',
          flexWrap: 'wrap',
          gap: 1.5,
          mb: 2,
          alignItems: 'center',
        }}
      >
        <TextField
          select
          size="small"
          sx={{ minWidth: 200 }}
          label={t('aliases.filters.dimension')}
          value={filters.dimension ?? ''}
          onChange={(event) =>
            setFilters({
              ...filters,
              dimension: event.target.value || undefined,
            })
          }
        >
          <MenuItem value="">
            <em>{t('aliases.filters.allDimensions')}</em>
          </MenuItem>
          <MenuItem value={deviceDimension}>
            {t('aliases.dimension.devices')}
          </MenuItem>
          {dimensions.map((name) => (
            <MenuItem key={name} value={parameterDimension(name)}>
              {t('aliases.dimension.parameterNamed', { name })}
            </MenuItem>
          ))}
        </TextField>
        <SearchField
          value={filters.search ?? ''}
          onChange={(search) => setFilters({ ...filters, search }, true)}
        />
        <FormControlLabel
          control={
            <Switch
              checked={filters.removed}
              onChange={(event) =>
                setFilters({ ...filters, removed: event.target.checked })
              }
            />
          }
          label={t('aliases.filters.showRemoved')}
        />
      </Box>

      <Box sx={{ height: 4, mb: 1 }}>
        {aliases.isFetching && (
          <LinearProgress aria-label={t('common.loading')} />
        )}
      </Box>

      {aliases.isError && !aliases.data ? (
        <ErrorAlert
          message={t('aliases.error', { message: aliases.error.message })}
          onRetry={() => aliases.refetch()}
        />
      ) : !aliases.data ? null : shown.length === 0 ? (
        <EmptyState>
          {rows.some((r) => r.target !== undefined) || filters.removed
            ? t('aliases.noMatch')
            : t('aliases.empty')}
        </EmptyState>
      ) : (
        <>
          <AliasTable
            rows={shown}
            all={rows}
            isKnown={known ? (row) => isKnown(known, row) : () => true}
            onEdit={(row) => openDialog({ kind: 'edit', row })}
            onRemove={setRemoving}
          />
          <Typography variant="body2" color="text.secondary" sx={{ my: 1.5 }}>
            {t('aliases.count', { count: shown.length })}
          </Typography>
        </>
      )}

      <AliasDialog
        key={dialog.key}
        open={dialog.open}
        mode={dialog.mode}
        rows={rows}
        known={known}
        onClose={() => setDialog((d) => ({ ...d, open: false }))}
      />
      <RemoveAliasDialog
        row={removing}
        onClose={() => setRemoving(undefined)}
      />
    </>
  )
}

/**
 * The text filter. It keeps what is typed (the URL drops surrounding
 * spaces) and follows the URL when it changes elsewhere (back button).
 */
function SearchField({
  value,
  onChange,
}: {
  value: string
  onChange: (value: string) => void
}) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState(value)
  const [shown, setShown] = useState(value)
  if (value !== shown) {
    setShown(value)
    if (value !== draft.trim()) setDraft(value)
  }
  return (
    <TextField
      size="small"
      type="search"
      sx={{ minWidth: 200, flex: '0 1 280px' }}
      label={t('aliases.filters.search')}
      value={draft}
      onChange={(event) => {
        setDraft(event.target.value)
        onChange(event.target.value)
      }}
    />
  )
}

type TableProps = {
  rows: readonly AliasRow[]
  /** All aliases, to follow chains. */
  all: readonly AliasRow[]
  isKnown: (row: AliasRow) => boolean
  onEdit: (row: AliasRow) => void
  onRemove: (row: AliasRow) => void
}

function AliasTable({ rows, all, isKnown, onEdit, onRemove }: TableProps) {
  const { t } = useTranslation()
  const label = useValueLabel()
  return (
    <TableContainer component={Paper} variant="outlined">
      <Table size="small" aria-label={t('nav.items.aliases')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('aliases.columns.dimension')}</TableCell>
            <TableCell>{t('aliases.columns.source')}</TableCell>
            <TableCell padding="none" />
            <TableCell>{t('aliases.columns.target')}</TableCell>
            <TableCell>{t('aliases.columns.version')}</TableCell>
            <TableCell>{t('aliases.columns.updated')}</TableCell>
            <TableCell align="right">
              <Box component="span" sx={visuallyHidden}>
                {t('aliases.columns.actions')}
              </Box>
            </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => {
            const dimension = parseDimension(row.dimension)
            const typed = dimension?.kind === 'parameter'
            const removed = row.target === undefined
            const final =
              row.target === undefined
                ? undefined
                : resolve(all, row.dimension, row.source)
            const source = label(row.source, typed)
            return (
              <TableRow key={row.id} sx={{ opacity: removed ? 0.6 : 1 }}>
                <TableCell>
                  {dimension?.kind === 'parameter' ? (
                    <>
                      {t('aliases.dimension.parameter')}{' '}
                      <Box component="code" sx={{ fontFamily: 'monospace' }}>
                        {dimension.name}
                      </Box>
                    </>
                  ) : dimension?.kind === 'device' ? (
                    t('aliases.dimension.device')
                  ) : (
                    row.dimension
                  )}
                </TableCell>
                <TableCell>
                  <Box
                    sx={{
                      display: 'flex',
                      gap: 1,
                      alignItems: 'center',
                      flexWrap: 'wrap',
                    }}
                  >
                    <ValueLabel value={row.source} typed={typed} />
                    {!removed && !isKnown(row) && (
                      <Tooltip title={t('aliases.notInResultsHint')}>
                        <Chip
                          size="small"
                          variant="outlined"
                          color="warning"
                          label={t('aliases.notInResults')}
                        />
                      </Tooltip>
                    )}
                  </Box>
                </TableCell>
                <TableCell padding="none" sx={{ color: 'text.secondary' }}>
                  <ArrowForwardIcon
                    fontSize="small"
                    sx={{ verticalAlign: 'middle' }}
                  />
                </TableCell>
                <TableCell>
                  {row.target === undefined ? (
                    <Chip size="small" label={t('aliases.removed')} />
                  ) : (
                    <Box>
                      <ValueLabel value={row.target} typed={typed} />
                      {final !== undefined && !sameValue(final, row.target) && (
                        <Typography variant="body2" color="text.secondary">
                          {t('aliases.resolvesTo', {
                            value: label(final, typed),
                          })}
                        </Typography>
                      )}
                    </Box>
                  )}
                </TableCell>
                <TableCell>
                  {t('aliases.version', { version: row.version })}
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Tooltip describeChild title={formatTimestamp(row.updatedAt)}>
                    <time dateTime={row.updatedAt}>
                      {formatDateTime(row.updatedAt)}
                    </time>
                  </Tooltip>
                </TableCell>
                <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                  {removed ? (
                    <Tooltip title={t('aliases.restore')}>
                      <IconButton
                        size="small"
                        aria-label={t('aliases.restoreNamed', { source })}
                        onClick={() => onEdit(row)}
                      >
                        <RestoreIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  ) : (
                    <>
                      <Tooltip title={t('aliases.edit')}>
                        <IconButton
                          size="small"
                          aria-label={t('aliases.editNamed', { source })}
                          onClick={() => onEdit(row)}
                        >
                          <EditOutlinedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title={t('aliases.remove')}>
                        <IconButton
                          size="small"
                          aria-label={t('aliases.removeNamed', { source })}
                          onClick={() => onRemove(row)}
                        >
                          <DeleteOutlinedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                    </>
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

/** Removes an alias: a new version without target, so it can be set again. */
function RemoveAliasDialog({
  row,
  onClose,
}: {
  row: AliasRow | undefined
  onClose: () => void
}) {
  const { t } = useTranslation()
  const label = useValueLabel()
  const setAlias = useSetAlias()
  // Keeps the text while the dialog fades out after `row` is cleared.
  const [shown, setShown] = useState(row)
  if (row && row !== shown) setShown(row)
  const typed = shown
    ? parseDimension(shown.dimension)?.kind === 'parameter'
    : false

  return (
    <ConfirmDialog
      open={row !== undefined}
      title={t('aliases.removeDialog.title')}
      confirmLabel={t('aliases.removeDialog.confirm')}
      pending={setAlias.isPending}
      error={setAlias.error}
      onClose={onClose}
      onExited={() => setAlias.reset()}
      onConfirm={() => {
        if (!row) return
        setAlias.mutate(
          { dimension: row.dimension, source: row.source, target: null },
          { onSuccess: onClose },
        )
      }}
    >
      {shown?.target !== undefined && (
        <Typography>
          {t('aliases.removeDialog.text', {
            source: label(shown.source, typed),
            target: label(shown.target, typed),
          })}
        </Typography>
      )}
    </ConfirmDialog>
  )
}
