import type { CSSProperties } from 'react'
import { useTranslation } from 'react-i18next'
import { Alert, Button } from '../ui'

type Props = {
  message: string
  onRetry: () => void
  style?: CSSProperties
}

/** A failed request, with a button to try again. */
export function ErrorAlert({ message, onRetry, style }: Props) {
  const { t } = useTranslation()
  return (
    <Alert
      tone="error"
      style={style}
      action={
        <Button variant="ghost" size="sm" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      }
    >
      {message}
    </Alert>
  )
}
