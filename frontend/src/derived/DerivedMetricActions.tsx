import DeleteIcon from '@mui/icons-material/DeleteOutlined'
import RenameIcon from '@mui/icons-material/DriveFileRenameOutline'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { DerivedMetric } from '../api/client'
import { derivedReference, isValidDerivedName } from '../dsl/syntax'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import type { SavedState } from './DerivedMetricDetail'
import {
  type DerivedView,
  derivedUsers,
  type SetDerivedView,
} from './derivedModel'
import { useDeleteDerivedMetric, useRenameDerivedMetric } from './queries'

type Props = {
  metric: DerivedMetric
  metrics: readonly DerivedMetric[]
  view: DerivedView
  setView: SetDerivedView
}

/** "Rename" and "Delete" for a derived metric, each asking first (ADR 0017). */
export function DerivedMetricActions(props: Props) {
  return (
    <>
      <RenameAction {...props} />
      <DeleteAction {...props} />
    </>
  )
}

const code = (text: string) => (
  <Box component="code" sx={{ fontFamily: 'monospace' }}>
    {text}
  </Box>
)

function RenameAction({ metric, metrics, view, setView }: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [newName, setNewName] = useState('')
  const rename = useRenameDerivedMetric()
  const users = useMemo(
    () => derivedUsers(metrics, metric.name),
    [metrics, metric.name],
  )

  const trimmed = newName.trim()
  const error =
    newName === ''
      ? undefined
      : !isValidDerivedName(newName)
        ? t('derived.editor.nameInvalid')
        : newName === metric.name
          ? t('derived.rename.same')
          : metrics.some((m) => m.name === newName)
            ? t('derived.rename.taken')
            : undefined

  return (
    <>
      <Button
        size="small"
        variant="outlined"
        startIcon={<RenameIcon />}
        onClick={() => {
          setNewName(metric.name)
          setOpen(true)
        }}
      >
        {t('derived.rename.action')}
      </Button>
      <ConfirmDialog
        open={open}
        title={t('derived.rename.title', { name: metric.name })}
        confirmLabel={t('derived.rename.confirm')}
        canConfirm={trimmed !== '' && error === undefined}
        pending={rename.isPending}
        error={rename.error}
        onClose={() => setOpen(false)}
        onExited={() => rename.reset()}
        onConfirm={() =>
          rename.mutate(
            { name: metric.name, newName },
            {
              onSuccess: (renamed) => {
                setOpen(false)
                const state: SavedState = {
                  renamed: {
                    from: metric.name,
                    name: renamed.name,
                    rewritten: renamed.rewritten,
                  },
                }
                // The old name no longer exists: replace its history entry.
                setView(
                  { ...view, name: renamed.name, edit: undefined },
                  { replace: true, state },
                )
              },
            },
          )
        }
      >
        <Box sx={{ display: 'grid', gap: 2, pt: 1 }}>
          <TextField
            label={t('derived.rename.newName')}
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            error={error !== undefined}
            helperText={error ?? t('derived.rename.help')}
            slotProps={{
              htmlInput: { spellCheck: false, sx: { fontFamily: 'monospace' } },
            }}
          />
          {users.length > 0 && (
            <Typography variant="body2">
              {t('derived.rename.users', { count: users.length })}{' '}
              {code(users.join(', '))}
            </Typography>
          )}
          <Alert severity="warning">
            {t('derived.rename.views')} {code(derivedReference(metric.name))}
          </Alert>
        </Box>
      </ConfirmDialog>
    </>
  )
}

function DeleteAction({ metric, metrics, view, setView }: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const remove = useDeleteDerivedMetric()
  const users = useMemo(
    () => derivedUsers(metrics, metric.name),
    [metrics, metric.name],
  )

  return (
    <>
      <Button
        size="small"
        variant="outlined"
        color="error"
        startIcon={<DeleteIcon />}
        onClick={() => setOpen(true)}
      >
        {t('derived.delete.action')}
      </Button>
      <ConfirmDialog
        open={open}
        title={t('derived.delete.title', { name: metric.name })}
        confirmLabel={t('derived.delete.confirm')}
        destructive
        canConfirm={users.length === 0}
        pending={remove.isPending}
        error={remove.error}
        onClose={() => setOpen(false)}
        onExited={() => remove.reset()}
        onConfirm={() =>
          remove.mutate(metric.name, {
            onSuccess: () => {
              setOpen(false)
              const state: SavedState = { deleted: metric.name }
              setView(
                {
                  search: view.search,
                  run: view.run,
                  caseExecution: view.caseExecution,
                },
                { replace: true, state },
              )
            },
          })
        }
      >
        {users.length > 0 ? (
          <Alert severity="info">
            {t('derived.delete.inUse', { count: users.length })}{' '}
            {code(users.join(', '))}
          </Alert>
        ) : (
          <Typography variant="body2">{t('derived.delete.text')}</Typography>
        )}
      </ConfirmDialog>
    </>
  )
}
