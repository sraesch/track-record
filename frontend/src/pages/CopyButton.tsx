import { Check, Copy } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { IconButton } from '../ui'

type Props = {
  text: string
  /** What the button does, e.g. "Copy run ID". */
  label: string
}

/** Copies a value (an ID, a commit) to the clipboard. */
export function CopyButton({ text, label }: Props) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!copied) return
    const timer = setTimeout(() => setCopied(false), 1500)
    return () => clearTimeout(timer)
  }, [copied])

  // The clipboard API is only there on HTTPS and localhost.
  if (!navigator.clipboard) return null

  return (
    <IconButton
      size="sm"
      icon={copied ? Check : Copy}
      label={label}
      tooltip={copied ? t('common.copied') : label}
      onClick={async () => {
        await navigator.clipboard.writeText(text)
        setCopied(true)
      }}
    />
  )
}
