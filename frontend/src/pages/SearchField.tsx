import TextField from '@mui/material/TextField'
import { useState } from 'react'

type Props = {
  label: string
  /** The search as it is in the URL (trimmed). */
  value: string
  onChange: (value: string) => void
}

/**
 * A text filter kept in the URL. It keeps what is typed (the URL drops
 * surrounding spaces) and follows the URL when it changes elsewhere (back
 * button).
 */
export function SearchField({ label, value, onChange }: Props) {
  const [draft, setDraft] = useState(value)
  const [shown, setShown] = useState(value)
  if (value !== shown) {
    setShown(value)
    if (value !== draft.trim()) setDraft(value)
  }
  return (
    <TextField
      size="small"
      type="search"
      sx={{ minWidth: 200, flex: '0 1 280px' }}
      label={label}
      value={draft}
      onChange={(event) => {
        setDraft(event.target.value)
        onChange(event.target.value)
      }}
    />
  )
}
