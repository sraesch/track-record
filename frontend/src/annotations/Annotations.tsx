import { Pencil, Trash2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Annotation } from '../api/client'
import { formatDateTime, formatTimestamp } from '../format'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { ErrorAlert } from '../pages/ErrorAlert'
import { Section } from '../pages/Section'
import {
  Alert,
  Button,
  IconButton,
  ProgressBar,
  space,
  Stack,
  TextArea,
  Tooltip,
} from '../ui'
import {
  type AnnotationTarget,
  useCreateAnnotation,
  useDeleteAnnotation,
  useUpdateAnnotation,
} from './queries'
import './annotations.css'

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
      <Stack gap={3}>
        {description && (
          <p className="ui-text small ui-secondary">{description}</p>
        )}
        {annotations === undefined ? (
          loadError ? (
            <ErrorAlert
              message={t('annotations.loadError', {
                message: loadError.message,
              })}
              onRetry={loadError.onRetry}
            />
          ) : (
            <ProgressBar />
          )
        ) : annotations.length === 0 ? (
          <p className="ui-text small ui-secondary">{t('annotations.empty')}</p>
        ) : (
          <Stack as="ul" gap={2} aria-label={title} className="ui-list">
            {annotations.map((annotation) => (
              <AnnotationItem
                key={annotation.id}
                annotation={annotation}
                onDelete={() => setDeleting(annotation)}
              />
            ))}
          </Stack>
        )}

        <form
          className="ui-stack"
          style={{ gap: space(2) }}
          noValidate
          onSubmit={(event) => {
            event.preventDefault()
            if (!body || create.isPending) return
            create.mutate(body, { onSuccess: () => setDraft('') })
          }}
        >
          <TextArea
            rows={2}
            label={t('annotations.new')}
            value={draft}
            onChange={(event) => {
              setDraft(event.target.value)
              if (create.isError) create.reset()
            }}
          />
          <Stack row justify="flex-end">
            <Button
              type="submit"
              size="sm"
              disabled={!body}
              loading={create.isPending}
            >
              {t('annotations.add')}
            </Button>
          </Stack>
          {create.error && (
            <Alert tone="error">
              {t('annotations.error', { message: create.error.message })}
            </Alert>
          )}
        </form>
      </Stack>

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
        <p className="ui-text">{t('annotations.deleteText')}</p>
        <blockquote className="tr-annotations-quote">
          {deleting?.body}
        </blockquote>
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
    <li className="tr-annotations-item">
      <div className="ui-grow">
        <p className="ui-text tr-annotations-body">{annotation.body}</p>
        <span className="small ui-secondary">
          <Tooltip title={formatTimestamp(annotation.created_at)}>
            <time dateTime={annotation.created_at}>
              {formatDateTime(annotation.created_at)}
            </time>
          </Tooltip>
          {edited && (
            <Tooltip title={formatTimestamp(annotation.updated_at)}>
              <span> {t('annotations.edited')}</span>
            </Tooltip>
          )}
        </span>
      </div>
      <div className="tr-annotations-actions">
        <IconButton
          size="sm"
          icon={Pencil}
          label={t('annotations.edit')}
          onClick={() => setEditing(true)}
        />
        <IconButton
          size="sm"
          icon={Trash2}
          label={t('annotations.delete')}
          onClick={onDelete}
        />
      </div>
    </li>
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
    <li className="tr-annotations-item">
      <form
        className="ui-stack ui-grow"
        style={{ gap: space(2) }}
        noValidate
        onSubmit={(event) => {
          event.preventDefault()
          if (!body || update.isPending) return
          if (body === annotation.body) return onDone()
          update.mutate({ annotation, body }, { onSuccess: onDone })
        }}
      >
        <TextArea
          rows={2}
          ref={inputRef}
          label={t('annotations.editLabel')}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && !update.isPending) onDone()
          }}
        />
        {update.error && (
          <Alert tone="error">
            {t('annotations.error', { message: update.error.message })}
          </Alert>
        )}
        <Stack row gap={2} justify="flex-end">
          <Button
            variant="ghost"
            size="sm"
            disabled={update.isPending}
            onClick={onDone}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            variant="primary"
            size="sm"
            disabled={!body}
            loading={update.isPending}
          >
            {t('common.save')}
          </Button>
        </Stack>
      </form>
    </li>
  )
}
