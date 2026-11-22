import Box from '@mui/material/Box'
import Skeleton from '@mui/material/Skeleton'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { lazy, Suspense, useId } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorBoundary } from '../pages/ErrorBoundary'
import type { DslEditorProps } from './DslEditor'
import type { DslNames, DslProblem } from './syntax'

// CodeMirror is loaded with the first expression editor a page shows, as
// its own chunk (ADR 0016, vite.config.ts).
const DslEditor = lazy(() =>
  import('./DslEditor').then((module) => ({ default: module.DslEditor })),
)

type Props = {
  label: string
  value: string
  onChange: (value: string) => void
  problems: readonly DslProblem[]
  names: DslNames
  /** Help below the editor. */
  helperText?: string
  /** Id of more elements that describe the field, e.g. its problems. */
  describedBy?: string
  invalid?: boolean
}

/**
 * A DSL expression with a visible label (FR-17a). If the editor cannot be
 * loaded (e.g. after a release replaced its chunk), a plain text field
 * takes its place, so the expression can still be edited.
 */
export function ExpressionField({
  label,
  value,
  onChange,
  problems,
  names,
  helperText,
  describedBy,
  invalid,
}: Props) {
  const { t } = useTranslation()
  const labelId = useId()
  const helperId = useId()
  const describedByIds =
    [helperText ? helperId : undefined, describedBy]
      .filter(Boolean)
      .join(' ') || undefined

  const editorProps: DslEditorProps = {
    value,
    onChange,
    problems,
    names,
    labelledBy: labelId,
    describedBy: describedByIds,
    invalid,
    kindLabel: (kind) => t(`dsl.kinds.${kind}`),
    functionInfo: (name) => t(`dsl.functions.${name}`),
  }

  return (
    <Box sx={{ minWidth: 0 }}>
      <Typography
        id={labelId}
        component="div"
        variant="body2"
        color={invalid ? 'error' : 'text.secondary'}
        sx={{ mb: 0.5 }}
      >
        {label}
      </Typography>
      <ErrorBoundary
        fallback={() => (
          <TextField
            multiline
            fullWidth
            minRows={3}
            value={value}
            error={invalid}
            onChange={(event) => onChange(event.target.value)}
            slotProps={{
              htmlInput: {
                'aria-labelledby': labelId,
                'aria-describedby': describedByIds,
                spellCheck: false,
                sx: { fontFamily: 'monospace' },
              },
            }}
          />
        )}
      >
        <Suspense
          fallback={
            <Skeleton
              variant="rounded"
              height={86}
              aria-label={t('common.loading')}
            />
          }
        >
          <DslEditor {...editorProps} />
        </Suspense>
      </ErrorBoundary>
      {helperText && (
        <Typography
          id={helperId}
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', mt: 0.5, mx: 1.75 }}
        >
          {helperText}
        </Typography>
      )}
    </Box>
  )
}
