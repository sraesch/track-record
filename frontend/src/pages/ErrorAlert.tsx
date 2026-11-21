import Alert from '@mui/material/Alert'
import Button from '@mui/material/Button'
import type { SxProps, Theme } from '@mui/material/styles'
import { useTranslation } from 'react-i18next'

type Props = {
  message: string
  onRetry: () => void
  sx?: SxProps<Theme>
}

/** A failed request, with a button to try again. */
export function ErrorAlert({ message, onRetry, sx }: Props) {
  const { t } = useTranslation()
  return (
    <Alert
      severity="error"
      sx={sx}
      action={
        <Button color="inherit" size="small" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      }
    >
      {message}
    </Alert>
  )
}
