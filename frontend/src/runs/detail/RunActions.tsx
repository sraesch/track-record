import { TextField } from '@graphite/react'
import { Ban, Trash } from 'lucide-react'
import { useState } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { RunDetail } from '../../api/client'
import { ConfirmDialog } from '../../pages/ConfirmDialog'
import { Button, Stack, TextArea } from '../../ui'
import { useDeleteRun, useInvalidateRun } from './mutations'

/**
 * Actions on a whole run, only in the web UI (no token reaches them): mark
 * it invalid (FR-9) and delete it (FR-10). Marking it valid again is next
 * to the notice that it is invalid.
 */
export function RunActions({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const [dialog, setDialog] = useState<'invalidate' | 'delete' | null>(null)
  const close = () => setDialog(null)
  return (
    <Stack row gap={2}>
      {run.is_valid && (
        <Button
          className="ui-nowrap"
          icon={Ban}
          onClick={() => setDialog('invalidate')}
        >
          {t('runDetail.invalidate.action')}
        </Button>
      )}
      <Button
        className="ui-nowrap"
        variant="danger"
        icon={Trash}
        onClick={() => setDialog('delete')}
      >
        {t('runDetail.delete.action')}
      </Button>
      <InvalidateDialog
        runId={run.id}
        open={dialog === 'invalidate'}
        onClose={close}
      />
      <DeleteDialog runId={run.id} open={dialog === 'delete'} onClose={close} />
    </Stack>
  )
}

type DialogProps = {
  runId: string
  open: boolean
  onClose: () => void
}

function InvalidateDialog({ runId, open, onClose }: DialogProps) {
  const { t } = useTranslation()
  const invalidate = useInvalidateRun(runId)
  const [reason, setReason] = useState('')
  return (
    <ConfirmDialog
      open={open}
      title={t('runDetail.invalidate.title')}
      confirmLabel={t('runDetail.invalidate.confirm')}
      canConfirm={reason.trim() !== ''}
      pending={invalidate.isPending}
      error={invalidate.error}
      onConfirm={() => invalidate.mutate(reason.trim(), { onSuccess: onClose })}
      onClose={onClose}
      onExited={() => {
        setReason('')
        invalidate.reset()
      }}
    >
      <p className="ui-text">{t('runDetail.invalidate.text')}</p>
      <TextArea
        rows={2}
        label={t('runDetail.invalidate.reason')}
        value={reason}
        onChange={(event) => setReason(event.target.value)}
        onKeyDown={(event) => {
          // Enter confirms, as in a single-line field; Shift+Enter for a new line.
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault()
            event.currentTarget.closest('form')?.requestSubmit()
          }
        }}
      />
    </ConfirmDialog>
  )
}

function DeleteDialog({ runId, open, onClose }: DialogProps) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const remove = useDeleteRun(runId)
  const [confirmation, setConfirmation] = useState('')
  return (
    <ConfirmDialog
      open={open}
      title={t('runDetail.delete.title')}
      confirmLabel={t('runDetail.delete.confirm')}
      destructive
      canConfirm={confirmation.trim() === runId}
      pending={remove.isPending}
      error={remove.error}
      onConfirm={() =>
        remove.mutate(confirmation.trim(), {
          onSuccess: () => navigate('/runs', { replace: true }),
        })
      }
      onClose={onClose}
      onExited={() => {
        setConfirmation('')
        remove.reset()
      }}
    >
      <p className="ui-text">{t('runDetail.delete.text')}</p>
      <p className="ui-text">
        <Trans
          i18nKey="runDetail.delete.prompt"
          values={{ runId }}
          components={{
            code: <code className="ui-code" style={{ fontWeight: 600 }} />,
          }}
        />
      </p>
      <TextField
        className="ui-mono-input"
        label={t('runDetail.delete.label')}
        value={confirmation}
        onChange={(event) => setConfirmation(event.target.value)}
        autoComplete="off"
        spellCheck={false}
      />
    </ConfirmDialog>
  )
}
