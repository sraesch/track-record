import AddIcon from '@mui/icons-material/Add'
import CancelIcon from '@mui/icons-material/Cancel'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import IconButton from '@mui/material/IconButton'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { RunDetail } from '../../api/client'
import { Section } from '../../pages/Section'
import { useAddRunTag, useRemoveRunTag } from './mutations'

/**
 * Tags of a run: add and remove them; a tag opens the run list filtered by
 * it. Tags are not results and can always be changed.
 */
export function RunTags({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const add = useAddRunTag(run.id)
  const remove = useRemoveRunTag(run.id)
  const [draft, setDraft] = useState('')
  const removeHintId = useId()
  const tag = draft.trim()
  const error = add.error ?? remove.error

  return (
    <Section title={t('runDetail.tags.title')}>
      {run.tags.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
          {t('runDetail.tags.empty')}
        </Typography>
      ) : (
        <>
          {/* The remove icon is inside the chip's button, so screen readers
              do not reach it; they hear that Delete removes the tag. */}
          <Box id={removeHintId} sx={visuallyHidden}>
            {t('runDetail.tags.removeHint')}
          </Box>
          <Box
            component="ul"
            aria-label={t('runDetail.tags.title')}
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
            {run.tags.map((name) => (
              <li key={name}>
                <Chip
                  size="small"
                  label={name}
                  aria-label={t('runDetail.tags.show', { tag: name })}
                  aria-describedby={removeHintId}
                  onClick={() =>
                    navigate(`/runs?${new URLSearchParams({ tag: name })}`)
                  }
                  onDelete={() => remove.mutate(name)}
                  deleteIcon={
                    <CancelIcon
                      titleAccess={t('runDetail.tags.remove', { tag: name })}
                    />
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
          label={t('runDetail.tags.new')}
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value)
            if (add.isError) add.reset()
          }}
          slotProps={{ htmlInput: { autoComplete: 'off' } }}
        />
        <IconButton
          type="submit"
          aria-label={t('runDetail.tags.add')}
          disabled={!tag}
          loading={add.isPending}
        >
          <AddIcon />
        </IconButton>
      </Box>
      {error && (
        <Alert severity="error" sx={{ mt: 1.5 }}>
          {t('runDetail.tags.error', { message: error.message })}
        </Alert>
      )}
    </Section>
  )
}

const visuallyHidden = {
  position: 'absolute',
  width: '1px',
  height: '1px',
  overflow: 'hidden',
  clip: 'rect(0 0 0 0)',
  whiteSpace: 'nowrap',
} as const
