import DeleteOutlinedIcon from '@mui/icons-material/DeleteOutlined'
import EditOutlinedIcon from '@mui/icons-material/EditOutlined'
import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import IconButton from '@mui/material/IconButton'
import LinearProgress from '@mui/material/LinearProgress'
import TextField from '@mui/material/TextField'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Annotation } from '../api/client'
import { formatDateTime, formatTimestamp } from '../format'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { ErrorAlert } from '../pages/ErrorAlert'
import { Section } from '../pages/Section'
import {
  type AnnotationTarget,
  useCreateAnnotation,
  useDeleteAnnotation,
  useUpdateAnnotation,
} from './queries'

type Props = {
  title: string
  /** What the annotations are on, e.g. that they apply to every run. */
  description?: string
  /** Undefined while they load or if loading failed. */
  annotations: readonly Annotation[] | undefined
  /** Where new annotations go. */
  target: AnnotationTarget
  /** A failed load, with a way to try again. */
  loadError?: { message: string; onRetry: () => void }
}

/**
 * Free-text notes on a run or a test case: add, edit and delete them. They
 * have no author.
 */
export function Annotations({
  title,
  description,
  annotations,
  target,
  loadError,
}: Props) {
  const { t } = useTranslation()
  const create = useCreateAnnotation(target)
  const [draft, setDraft] = useState('')
  const [deleting, setDeleting] = useState<Annotation | null>(null)
  const remove = useDeleteAnnotation()
  const body = draft.trim()

  return (
    <Section title={title}>
      {description && (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
          {description}
        </Typography>
      )}
      {annotations === undefined ? (
        loadError ? (
          <ErrorAlert
            message={t('annotations.loadError', { message: loadError.message })}
            onRetry={loadError.onRetry}
            sx={{ mb: 1.5 }}
          />
        ) : (
          <LinearProgress aria-label={t('common.loading')} sx={{ mb: 1.5 }} />
        )
      ) : annotations.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
          {t('annotations.empty')}
        </Typography>
      ) : (
        <Box
          component="ul"
          aria-label={title}
          sx={{ m: 0, mb: 1.5, p: 0, listStyle: 'none', display: 'grid', gap: 1 }}
        >
          {annotations.map((annotation) => (
            <AnnotationItem
              key={annotation.id}
              annotation={annotation}
              onDelete={() => setDeleting(annotation)}
            />
          ))}
        </Box>
      )}

      <Box
        component="form"
        noValidate
        onSubmit={(event) => {
          event.preventDefault()
          if (!body || create.isPending) return
          create.mutate(body, { onSuccess: () => setDraft('') })
        }}
      >
        <TextField
          size="small"
          fullWidth
          multiline
          minRows={2}
          label={t('annotations.new')}
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value)
            if (create.isError) create.reset()
          }}
        />
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 1 }}>
          <Button
            type="submit"
            size="small"
            variant="outlined"
            disabled={!body}
            loading={create.isPending}
          >
            {t('annotations.add')}
          </Button>
        </Box>
        {create.error && (
          <Alert severity="error" sx={{ mt: 1 }}>
            {t('annotations.error', { message: create.error.message })}
          </Alert>
        )}
      </Box>

      <ConfirmDialog
        open={deleting !== null}
        title={t('annotations.deleteTitle')}
        confirmLabel={t('common.delete')}
        destructive
        pending={remove.isPending}
        error={remove.error}
        onConfirm={() =>
          deleting &&
          remove.mutate(deleting, { onSuccess: () => setDeleting(null) })
        }
        onClose={() => setDeleting(null)}
        onExited={() => remove.reset()}
      >
        <Typography sx={{ mb: 1 }}>{t('annotations.deleteText')}</Typography>
        <Typography
          component="blockquote"
          color="text.secondary"
          sx={{
            m: 0,
            pl: 1.5,
            borderLeft: 3,
            borderColor: 'divider',
            whiteSpace: 'pre-wrap',
          }}
        >
          {deleting?.body}
        </Typography>
      </ConfirmDialog>
    </Section>
  )
}

type ItemProps = {
  annotation: Annotation
  onDelete: () => void
}

function AnnotationItem({ annotation, onDelete }: ItemProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const edited = annotation.updated_at !== annotation.created_at

  if (editing) {
    return (
      <AnnotationEditor
        annotation={annotation}
        onDone={() => setEditing(false)}
      />
    )
  }

  return (
    <Box
      component="li"
      sx={{ display: 'flex', gap: 1, p: 1, borderRadius: 1, bgcolor: 'action.hover' }}
    >
      <Box sx={{ flexGrow: 1, minWidth: 0 }}>
        <Typography
          variant="body2"
          sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}
        >
          {annotation.body}
        </Typography>
        <Typography variant="caption" color="text.secondary">
          <Tooltip describeChild title={formatTimestamp(annotation.created_at)}>
            <time dateTime={annotation.created_at}>
              {formatDateTime(annotation.created_at)}
            </time>
          </Tooltip>
          {edited && (
            <Tooltip describeChild title={formatTimestamp(annotation.updated_at)}>
              <span> {t('annotations.edited')}</span>
            </Tooltip>
          )}
        </Typography>
      </Box>
      <Box sx={{ display: 'flex', alignItems: 'flex-start', flexShrink: 0 }}>
        <Tooltip title={t('annotations.edit')}>
          <IconButton size="small" onClick={() => setEditing(true)}>
            <EditOutlinedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title={t('annotations.delete')}>
          <IconButton size="small" onClick={onDelete}>
            <DeleteOutlinedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  )
}

type EditorProps = {
  annotation: Annotation
  onDone: () => void
}

/** Edits an annotation in place; Escape or Cancel leaves it unchanged. */
function AnnotationEditor({ annotation, onDone }: EditorProps) {
  const { t } = useTranslation()
  const update = useUpdateAnnotation()
  const [draft, setDraft] = useState(annotation.body)
  const body = draft.trim()
  const inputRef = useRef<HTMLTextAreaElement>(null)

  // The edit button that opened the editor is gone: move the focus into
  // the text, at its end.
  useEffect(() => {
    const input = inputRef.current
    input?.focus()
    input?.setSelectionRange(input.value.length, input.value.length)
  }, [])

  return (
    <Box component="li" sx={{ p: 1, borderRadius: 1, bgcolor: 'action.hover' }}>
      <Box
        component="form"
        noValidate
        onSubmit={(event) => {
          event.preventDefault()
          if (!body || update.isPending) return
          if (body === annotation.body) return onDone()
          update.mutate({ annotation, body }, { onSuccess: onDone })
        }}
      >
        <TextField
          size="small"
          fullWidth
          multiline
          minRows={2}
          inputRef={inputRef}
          label={t('annotations.editLabel')}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && !update.isPending) onDone()
          }}
        />
        {update.error && (
          <Alert severity="error" sx={{ mt: 1 }}>
            {t('annotations.error', { message: update.error.message })}
          </Alert>
        )}
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, mt: 1 }}>
          <Button size="small" disabled={update.isPending} onClick={onDone}>
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            size="small"
            variant="contained"
            disabled={!body}
            loading={update.isPending}
          >
            {t('common.save')}
          </Button>
        </Box>
      </Box>
    </Box>
  )
}
