import Alert from '@mui/material/Alert'
import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogActions from '@mui/material/DialogActions'
import DialogContent from '@mui/material/DialogContent'
import DialogTitle from '@mui/material/DialogTitle'
import { type ReactNode, useId } from 'react'
import { useTranslation } from 'react-i18next'

type Props = {
  open: boolean
  title: string
  children: ReactNode
  confirmLabel: string
  onConfirm: () => void
  onClose: () => void
  /** Called once the dialog has closed, to reset its inputs. */
  onExited?: () => void
  /** False while the inputs are not complete; disables confirming. */
  canConfirm?: boolean
  /** The action is under way: confirming and closing are disabled. */
  pending?: boolean
  /** The action failed; shown in the dialog so it can be retried. */
  error?: Error | null
  /** Red confirm button, for actions that cannot be undone. */
  destructive?: boolean
}

/**
 * Asks before an action, optionally with inputs (children). The dialog is a
 * form: Enter in a text field confirms.
 */
export function ConfirmDialog({
  open,
  title,
  children,
  confirmLabel,
  onConfirm,
  onClose,
  onExited,
  canConfirm = true,
  pending = false,
  error,
  destructive = false,
}: Props) {
  const { t } = useTranslation()
  const titleId = useId()
  return (
    <Dialog
      open={open}
      onClose={pending ? undefined : onClose}
      aria-labelledby={titleId}
      fullWidth
      maxWidth="sm"
      slotProps={{
        paper: {
          component: 'form',
          onSubmit: (event: React.FormEvent) => {
            event.preventDefault()
            if (canConfirm && !pending) onConfirm()
          },
        },
        transition: {
          // The first field gets the focus once the dialog is open, so it
          // can be typed into right away (the WAI-ARIA dialog pattern).
          onEntered: (node: HTMLElement) =>
            node.querySelector<HTMLElement>('input, textarea')?.focus(),
          onExited,
        },
      }}
    >
      <DialogTitle id={titleId}>{title}</DialogTitle>
      <DialogContent>
        {children}
        {error && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {error.message}
          </Alert>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onClose} disabled={pending}>
          {t('common.cancel')}
        </Button>
        <Button
          type="submit"
          variant="contained"
          color={destructive ? 'error' : 'primary'}
          disabled={!canConfirm}
          loading={pending}
        >
          {confirmLabel}
        </Button>
      </DialogActions>
    </Dialog>
  )
}
