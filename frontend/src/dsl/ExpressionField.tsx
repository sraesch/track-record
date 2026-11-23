import { lazy, Suspense, useId } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorBoundary } from '../pages/ErrorBoundary'
import { Skeleton } from '../ui'
import type { DslEditorProps } from './DslEditor'
import type { DslNames, DslProblem } from './syntax'

// CodeMirror is loaded with the first expression editor a page shows, as
// its own chunk (ADR 0016, vite.config.ts).
const DslEditor = lazy(() =>
  import('./DslEditor').then((module) => ({ default: module.DslEditor })),
)

/** Height of the editor with three lines, for its placeholder. */
const editorHeight = 70

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
 * A DSL expression with a visible label (FR-17a), laid out like a Graphite
 * field. If the editor cannot be loaded (e.g. after a release replaced its
 * chunk), a plain text area takes its place, so the expression can still
 * be edited.
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
    <div className="g-field">
      <div id={labelId} className="g-label">
        {label}
      </div>
      <ErrorBoundary
        fallback={() => (
          <span
            className="g-input ui-textarea ui-mono-input"
            {...(invalid ? { 'data-invalid': '' } : {})}
          >
            <textarea
              rows={3}
              value={value}
              onChange={(event) => onChange(event.target.value)}
              aria-labelledby={labelId}
              aria-describedby={describedByIds}
              aria-invalid={invalid || undefined}
              spellCheck={false}
            />
          </span>
        )}
      >
        <Suspense
          fallback={
            <Skeleton height={editorHeight} label={t('common.loading')} />
          }
        >
          <DslEditor {...editorProps} />
        </Suspense>
      </ErrorBoundary>
      {helperText && (
        <span id={helperId} className="g-help">
          {helperText}
        </span>
      )}
    </div>
  )
}
