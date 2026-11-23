import { Switch } from '@graphite/react'
import { ArrowRight, Pencil, Plus, RotateCcw, Trash2 } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { SearchField } from '../pages/SearchField'
import { Time } from '../pages/Time'
import {
  Button,
  Chip,
  IconButton,
  ProgressBar,
  Select,
  Table,
  Tooltip,
  VisuallyHidden,
} from '../ui'
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
import './aliases.css'

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
            variant="primary"
            icon={Plus}
            onClick={() => openDialog({ kind: 'add' })}
          >
            {t('aliases.add')}
          </Button>
        }
      />

      <search
        aria-label={t('aliases.filters.label')}
        className="tr-aliases-filters"
      >
        <Select
          className="tr-aliases-dimension"
          label={t('aliases.filters.dimension')}
          value={filters.dimension ?? ''}
          onChange={(dimension) =>
            setFilters({ ...filters, dimension: dimension || undefined })
          }
          options={[
            { value: '', label: t('aliases.filters.allDimensions') },
            { value: deviceDimension, label: t('aliases.dimension.devices') },
            ...dimensions.map((name) => ({
              value: parameterDimension(name),
              label: t('aliases.dimension.parameterNamed', { name }),
            })),
          ]}
        />
        <SearchField
          label={t('aliases.filters.search')}
          value={filters.search ?? ''}
          onChange={(search) => setFilters({ ...filters, search }, true)}
        />
        <Switch
          label={t('aliases.filters.showRemoved')}
          checked={filters.removed}
          onCheckedChange={(removed) => setFilters({ ...filters, removed })}
        />
      </search>

      <div className="tr-aliases-progress">
        {aliases.isFetching && <ProgressBar />}
      </div>

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
          <p className="ui-text ui-secondary small tr-aliases-count">
            {t('aliases.count', { count: shown.length })}
          </p>
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
    <Table framed aria-label={t('nav.items.aliases')}>
      <thead>
        <tr>
          <th>{t('aliases.columns.dimension')}</th>
          <th>{t('aliases.columns.source')}</th>
          <th className="tr-aliases-arrow" aria-hidden />
          <th>{t('aliases.columns.target')}</th>
          <th>{t('aliases.columns.version')}</th>
          <th>{t('aliases.columns.updated')}</th>
          <th className="g-r">
            <VisuallyHidden>{t('aliases.columns.actions')}</VisuallyHidden>
          </th>
        </tr>
      </thead>
      <tbody>
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
            <tr
              key={row.id}
              className={removed ? 'tr-aliases-removed' : undefined}
            >
              <td>
                {dimension?.kind === 'parameter' ? (
                  <>
                    {t('aliases.dimension.parameter')}{' '}
                    <code className="ui-code">{dimension.name}</code>
                  </>
                ) : dimension?.kind === 'device' ? (
                  t('aliases.dimension.device')
                ) : (
                  row.dimension
                )}
              </td>
              <td>
                <span className="tr-aliases-source">
                  <ValueLabel value={row.source} typed={typed} />
                  {!removed && !isKnown(row) && (
                    <Tooltip title={t('aliases.notInResultsHint')}>
                      <Chip tone="warning" outlined>
                        {t('aliases.notInResults')}
                      </Chip>
                    </Tooltip>
                  )}
                </span>
              </td>
              <td className="tr-aliases-arrow">
                <ArrowRight size={14} className="ui-secondary" aria-hidden />
              </td>
              <td>
                {row.target === undefined ? (
                  <Chip>{t('aliases.removed')}</Chip>
                ) : (
                  <div>
                    <ValueLabel value={row.target} typed={typed} />
                    {final !== undefined && !sameValue(final, row.target) && (
                      <p className="ui-text ui-secondary small">
                        {t('aliases.resolvesTo', {
                          value: label(final, typed),
                        })}
                      </p>
                    )}
                  </div>
                )}
              </td>
              <td>{t('aliases.version', { version: row.version })}</td>
              <td>
                <Time iso={row.updatedAt} />
              </td>
              <td className="g-r">
                {removed ? (
                  <IconButton
                    size="sm"
                    icon={RotateCcw}
                    label={t('aliases.restoreNamed', { source })}
                    tooltip={t('aliases.restore')}
                    onClick={() => onEdit(row)}
                  />
                ) : (
                  <>
                    <IconButton
                      size="sm"
                      icon={Pencil}
                      label={t('aliases.editNamed', { source })}
                      tooltip={t('aliases.edit')}
                      onClick={() => onEdit(row)}
                    />
                    <IconButton
                      size="sm"
                      icon={Trash2}
                      label={t('aliases.removeNamed', { source })}
                      tooltip={t('aliases.remove')}
                      onClick={() => onRemove(row)}
                    />
                  </>
                )}
              </td>
            </tr>
          )
        })}
      </tbody>
    </Table>
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
        <p className="ui-text">
          {t('aliases.removeDialog.text', {
            source: label(shown.source, typed),
            target: label(shown.target, typed),
          })}
        </p>
      )}
    </ConfirmDialog>
  )
}
