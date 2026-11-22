import BlockIcon from '@mui/icons-material/Block'
import DeleteForeverIcon from '@mui/icons-material/DeleteForever'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useState } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { RunDetail } from '../../api/client'
import { ConfirmDialog } from '../../pages/ConfirmDialog'
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
    <Box
      sx={{ display: 'flex', gap: 1, '& .MuiButton-root': { whiteSpace: 'nowrap' } }}
    >
      {run.is_valid && (
        <Button
          variant="outlined"
          size="small"
          startIcon={<BlockIcon />}
          onClick={() => setDialog('invalidate')}
        >
          {t('runDetail.invalidate.action')}
        </Button>
      )}
      <Button
        variant="outlined"
        size="small"
        color="error"
        startIcon={<DeleteForeverIcon />}
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
    </Box>
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
      onConfirm={() =>
        invalidate.mutate(reason.trim(), { onSuccess: onClose })
      }
      onClose={onClose}
      onExited={() => {
        setReason('')
        invalidate.reset()
      }}
    >
      <Typography sx={{ mb: 2 }}>{t('runDetail.invalidate.text')}</Typography>
      <TextField
        fullWidth
        multiline
        minRows={2}
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
      <Typography sx={{ mb: 2 }}>{t('runDetail.delete.text')}</Typography>
      <Typography sx={{ mb: 2 }}>
        <Trans
          i18nKey="runDetail.delete.prompt"
          values={{ runId }}
          components={{
            code: (
              <Box
                component="code"
                sx={{
                  fontFamily: 'monospace',
                  fontWeight: 600,
                  overflowWrap: 'anywhere',
                }}
              />
            ),
          }}
        />
      </Typography>
      <TextField
        fullWidth
        label={t('runDetail.delete.label')}
        value={confirmation}
        onChange={(event) => setConfirmation(event.target.value)}
        slotProps={{
          htmlInput: {
            autoComplete: 'off',
            spellCheck: false,
            sx: { fontFamily: 'monospace' },
          },
        }}
      />
    </ConfirmDialog>
  )
}
