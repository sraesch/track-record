import { TextField } from '@graphite/react'
import { Plus } from 'lucide-react'
import type { UseMutationResult } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Section } from '../pages/Section'
import { Alert, Chip, IconButton, Stack, VisuallyHidden } from '../ui'

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
      <Stack gap={3}>
        {description && (
          <p className="ui-text ui-secondary small">{description}</p>
        )}
        {tags.length === 0 ? (
          <p className="ui-text ui-secondary small">{t('tags.empty')}</p>
        ) : (
          <>
            <VisuallyHidden id={removeHintId}>
              {t('tags.removeHint')}
            </VisuallyHidden>
            <Stack
              as="ul"
              row
              wrap
              gap={2}
              aria-label={title}
              className="ui-list"
            >
              {tags.map((name) => (
                <li key={name}>
                  <Chip
                    aria-label={open.label(name)}
                    aria-describedby={removeHintId}
                    onClick={() => open.onOpen(name)}
                    onRemove={() => remove.mutate(name)}
                    removeLabel={t('tags.remove', { tag: name })}
                    disabled={remove.isPending && remove.variables === name}
                  >
                    {name}
                  </Chip>
                </li>
              ))}
            </Stack>
          </>
        )}
        <Stack
          as="form"
          row
          gap={2}
          align="flex-end"
          noValidate
          onSubmit={(event) => {
            event.preventDefault()
            if (!tag || add.isPending) return
            add.mutate(tag, { onSuccess: () => setDraft('') })
          }}
        >
          <TextField
            className="ui-grow"
            label={t('tags.new')}
            value={draft}
            autoComplete="off"
            onChange={(event) => {
              setDraft(event.target.value)
              if (add.isError) add.reset()
            }}
          />
          <IconButton
            type="submit"
            variant="secondary"
            icon={Plus}
            label={t('tags.add')}
            disabled={!tag}
            loading={add.isPending}
          />
        </Stack>
        {error && (
          <Alert tone="error">
            {t('tags.error', { message: error.message })}
          </Alert>
        )}
      </Stack>
    </Section>
  )
}
