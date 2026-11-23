import { TextField } from '@graphite/react'
import { PencilLine, Trash2 } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { DerivedMetric } from '../api/client'
import { derivedReference, isValidDerivedName } from '../dsl/syntax'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { Alert, Button } from '../ui'
import type { SavedState } from './DerivedMetricDetail'
import {
  type DerivedView,
  derivedUsers,
  type SetDerivedView,
} from './derivedModel'
import { useDeleteDerivedMetric, useRenameDerivedMetric } from './queries'
import './derived.css'

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

const code = (text: string) => <code className="ui-code">{text}</code>

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
        size="sm"
        icon={PencilLine}
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
        <TextField
          label={t('derived.rename.newName')}
          value={newName}
          onChange={(event) => setNewName(event.target.value)}
          error={error}
          help={t('derived.rename.help')}
          spellCheck={false}
          className="ui-mono-input"
        />
        {users.length > 0 && (
          <p className="ui-text">
            {t('derived.rename.users', { count: users.length })}{' '}
            {code(users.join(', '))}
          </p>
        )}
        <Alert tone="warning">
          {t('derived.rename.views')} {code(derivedReference(metric.name))}
        </Alert>
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
        size="sm"
        icon={Trash2}
        className="tr-derived-delete"
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
          <Alert tone="info">
            {t('derived.delete.inUse', { count: users.length })}{' '}
            {code(users.join(', '))}
          </Alert>
        ) : (
          <p className="ui-text">{t('derived.delete.text')}</p>
        )}
      </ConfirmDialog>
    </>
  )
}
