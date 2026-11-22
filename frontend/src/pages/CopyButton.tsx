import CheckIcon from '@mui/icons-material/Check'
import ContentCopyIcon from '@mui/icons-material/ContentCopy'
import IconButton from '@mui/material/IconButton'
import Tooltip from '@mui/material/Tooltip'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

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
    <Tooltip title={copied ? t('common.copied') : label}>
      <IconButton
        size="small"
        aria-label={label}
        onClick={async () => {
          await navigator.clipboard.writeText(text)
          setCopied(true)
        }}
        sx={{ my: -0.5 }}
      >
        {copied ? (
          <CheckIcon fontSize="inherit" color="success" />
        ) : (
          <ContentCopyIcon fontSize="inherit" />
        )}
      </IconButton>
    </Tooltip>
  )
}
