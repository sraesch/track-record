import AddIcon from '@mui/icons-material/Add'
import CancelIcon from '@mui/icons-material/Cancel'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import IconButton from '@mui/material/IconButton'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import type { UseMutationResult } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Section } from '../pages/Section'
import { visuallyHidden } from '../pages/visuallyHidden'

/** Adds or removes one tag, given its name. */
type TagMutation = UseMutationResult<unknown, Error, string>

type Props = {
  title: string
  /** What the tags are on, e.g. that they apply to every run. */
  description?: string
  tags: readonly string[]
  add: TagMutation
  remove: TagMutation
  /** What a click on a tag does, and how that is announced. */
  open: { label: (tag: string) => string; onOpen: (tag: string) => void }
}

/**
 * Tags of a run or a test case: add and remove them; a click opens what
 * the tag leads to. Tags are not results and can always be changed.
 */
export function Tags({ title, description, tags, add, remove, open }: Props) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState('')
  const removeHintId = useId()
  const tag = draft.trim()
  const error = add.error ?? remove.error

  return (
    <Section title={title}>
      {description && (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
          {description}
        </Typography>
      )}
      {tags.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
          {t('tags.empty')}
        </Typography>
      ) : (
        <>
          {/* The remove icon is inside the chip's button, so screen readers
              do not reach it; they hear that Delete removes the tag. */}
          <Box id={removeHintId} sx={visuallyHidden}>
            {t('tags.removeHint')}
          </Box>
          <Box
            component="ul"
            aria-label={title}
            sx={{
              display: 'flex',
              flexWrap: 'wrap',
              gap: 0.75,
              m: 0,
              mb: 1.5,
              p: 0,
              listStyle: 'none',
            }}
          >
            {tags.map((name) => (
              <li key={name}>
                <Chip
                  size="small"
                  label={name}
                  aria-label={open.label(name)}
                  aria-describedby={removeHintId}
                  onClick={() => open.onOpen(name)}
                  onDelete={() => remove.mutate(name)}
                  deleteIcon={
                    <CancelIcon titleAccess={t('tags.remove', { tag: name })} />
                  }
                  disabled={remove.isPending && remove.variables === name}
                />
              </li>
            ))}
          </Box>
        </>
      )}
      <Box
        component="form"
        noValidate
        onSubmit={(event) => {
          event.preventDefault()
          if (!tag || add.isPending) return
          add.mutate(tag, { onSuccess: () => setDraft('') })
        }}
        sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
      >
        <TextField
          size="small"
          fullWidth
          label={t('tags.new')}
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value)
            if (add.isError) add.reset()
          }}
          slotProps={{ htmlInput: { autoComplete: 'off' } }}
        />
        <IconButton
          type="submit"
          aria-label={t('tags.add')}
          disabled={!tag}
          loading={add.isPending}
        >
          <AddIcon />
        </IconButton>
      </Box>
      {error && (
        <Alert severity="error" sx={{ mt: 1.5 }}>
          {t('tags.error', { message: error.message })}
        </Alert>
      )}
    </Section>
  )
}
