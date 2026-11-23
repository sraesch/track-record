import { useTranslation } from 'react-i18next'
import type { ReactNode } from 'react'
import { Alert, Button, Dialog } from '../ui'

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
 * form: Enter in a text field confirms. The first field has the focus once
 * the dialog is open, so it can be typed into right away.
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
  return (
    <Dialog
      open={open}
      title={title}
      size="md"
      onClose={pending ? undefined : onClose}
      onExited={onExited}
      onSubmit={(event) => {
        event.preventDefault()
        if (canConfirm && !pending) onConfirm()
      }}
      actions={
        <>
          <Button variant="ghost" onClick={onClose} disabled={pending}>
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            variant={destructive ? 'danger' : 'primary'}
            disabled={!canConfirm}
            loading={pending}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children}
      {error && <Alert tone="error">{error.message}</Alert>}
    </Dialog>
  )
}
