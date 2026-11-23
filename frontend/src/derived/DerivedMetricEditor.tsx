import { TextField } from '@graphite/react'
import { CircleAlert, CircleCheck, LoaderCircle } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type {
  DerivedMetric,
  ValidateExpression,
  Validation,
} from '../api/client'
import { ExpressionField } from '../dsl/ExpressionField'
import {
  type DslNames,
  type DslProblem,
  isValidDerivedName,
  toProblems,
} from '../dsl/syntax'
import { useMetrics, useUnits } from '../metrics/queries'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { Facts } from '../pages/Facts'
import { Section } from '../pages/Section'
import { useDebouncedValue } from '../pages/useDebouncedValue'
import { Alert, Button, SuggestField, TextArea } from '../ui'
import type { SavedState } from './DerivedMetricDetail'
import {
  type DerivedView,
  derivedUsersDeep,
  type SetDerivedView,
} from './derivedModel'
import { EvaluationPreview } from './EvaluationPreview'
import { FunctionReference } from './FunctionReference'
import { useSaveDerivedMetric, useValidation } from './queries'
import './derived.css'

/** Pause in typing after which the expression is checked. */
export const validationDelay = 400

type Props = {
  /** The derived metric to change; none for a new one. */
  metric?: DerivedMetric
  metrics: readonly DerivedMetric[]
  view: DerivedView
  setView: SetDerivedView
}

type Draft = { name: string; expression: string; unit: string }

/**
 * Writes a new derived metric or changes one (FR-17, FR-17a). The
 * expression is checked by the server while typing: problems are
 * underlined and listed, and a valid expression shows its kind, unit,
 * inputs and the derived metrics it uses, and can be tried on a case
 * execution before saving.
 *
 * A derived metric has no versions (ADR 0017). Saving a change replaces
 * the definition: for every analysis, past runs included, and for the
 * derived metrics built on it, which the server checks too.
 */
export function DerivedMetricEditor({ metric, metrics, view, setView }: Props) {
  const { t } = useTranslation()
  const isNew = !metric
  const [name, setName] = useState('')
  const [expression, setExpression] = useState(metric?.expression ?? '')
  const [unit, setUnit] = useState(metric?.unit ?? '')
  const [description, setDescription] = useState(metric?.description ?? '')
  const [discarding, setDiscarding] = useState(false)
  const problemsId = useId()

  const metricName = metric?.name ?? name.trim()
  // "New" never replaces a derived metric: that is what "Edit" is for.
  const exists = isNew && metrics.some((m) => m.name === name.trim())
  const nameError =
    !isNew || name === ''
      ? undefined
      : !isValidDerivedName(name)
        ? t('derived.editor.nameInvalid')
        : exists
          ? t('derived.editor.nameExists')
          : undefined
  const users = useMemo(
    () => (metric ? derivedUsersDeep(metrics, metric.name) : []),
    [metrics, metric],
  )

  // The draft as it was when typing paused; a string, so it only changes
  // with the text.
  const draftKey = JSON.stringify({
    name: metricName,
    expression,
    unit: unit.trim(),
  } satisfies Draft)
  const checkedKey = useDebouncedValue(draftKey, validationDelay)
  const checked = useMemo(() => JSON.parse(checkedKey) as Draft, [checkedKey])
  const request: ValidateExpression | undefined = checked.expression.trim()
    ? {
        expression: checked.expression,
        unit: checked.unit || null,
        name:
          checked.name && isValidDerivedName(checked.name)
            ? checked.name
            : null,
      }
    : undefined
  const validation = useValidation(request)
  // The result belongs to the text on screen: typing has paused and the
  // answer is not the previous one kept while loading.
  const current =
    checkedKey === draftKey &&
    request !== undefined &&
    !validation.isPlaceholderData
      ? validation.data
      : undefined

  const problems: DslProblem[] = useMemo(
    () =>
      current && !current.valid
        ? toProblems(expression, current.diagnostics)
        : [],
    [current, expression],
  )

  const uploaded = useMetrics()
  const units = useUnits()
  const names: DslNames = useMemo(
    () => ({
      metrics: uploaded.data?.map((m) => m.name) ?? [],
      derived: metrics.map((m) => m.name).filter((n) => n !== metric?.name),
    }),
    [uploaded.data, metrics, metric?.name],
  )
  const unitOptions = useMemo(
    () => units.data?.map((u) => u.symbol) ?? [],
    [units.data],
  )

  const save = useSaveDerivedMetric()
  const savedUnit = unit.trim() || current?.unit || ''
  const dirty = isNew
    ? name !== '' || expression !== '' || unit !== '' || description !== ''
    : expression !== metric.expression ||
      unit !== metric.unit ||
      description !== metric.description
  const canSave =
    !!current?.valid &&
    metricName !== '' &&
    isValidDerivedName(metricName) &&
    (isNew ? !exists : dirty) &&
    !save.isPending

  const close = () =>
    setView({
      search: view.search,
      name: metric?.name,
      run: view.run,
      caseExecution: view.caseExecution,
    })

  const submit = () => {
    if (!canSave) return
    save.mutate(
      {
        name: metricName,
        body: { expression, unit: savedUnit, description },
      },
      {
        onSuccess: (saved) => {
          const state: SavedState = {
            saved: { name: saved.name, created: saved.created },
          }
          // Shows the saved metric. The editor's history entry is
          // replaced, so Back does not reopen it.
          setView(
            {
              search: view.search,
              name: saved.name,
              run: view.run,
              caseExecution: view.caseExecution,
            },
            { replace: true, state },
          )
        },
      },
    )
  }

  return (
    <Section
      title={
        isNew
          ? t('derived.editor.titleNew')
          : t('derived.editor.titleChange', { name: metric.name })
      }
    >
      <form
        noValidate
        className="tr-derived-form"
        onSubmit={(event) => {
          event.preventDefault()
          submit()
        }}
      >
        {!isNew && (
          <Alert tone="info">
            {t('derived.editor.changeNote')}
            {users.length > 0 && (
              <>
                {' '}
                {t('derived.editor.changeUsers', {
                  count: users.length,
                  names: users.join(', '),
                })}
              </>
            )}
          </Alert>
        )}
        {isNew && (
          <TextField
            label={t('derived.editor.name')}
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            error={nameError}
            help={t('derived.editor.nameHelp')}
            spellCheck={false}
            className="ui-mono-input"
          />
        )}

        <ExpressionField
          label={t('derived.editor.expression')}
          value={expression}
          onChange={setExpression}
          problems={problems}
          names={names}
          helperText={t('derived.editor.expressionHelp')}
          describedBy={problems.length > 0 ? problemsId : undefined}
          invalid={problems.length > 0}
        />

        <CheckStatus
          id={problemsId}
          empty={!expression.trim()}
          checking={!current && validation.isError === false}
          error={validation.isError ? validation.error : null}
          validation={current}
          problems={problems}
          expression={expression}
        />

        <div className="tr-derived-fields">
          <SuggestField
            label={t('derived.editor.unit')}
            value={unit}
            onChange={setUnit}
            options={unitOptions}
            placeholder={current?.valid ? (current.inferred_unit ?? '') : ''}
            help={
              current?.valid
                ? t('derived.editor.unitHelp', {
                    unit:
                      current.inferred_unit || t('metrics.unit.dimensionless'),
                  })
                : t('derived.editor.unitHelpPending')
            }
          />
          <TextArea
            label={t('derived.editor.description')}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            rows={1}
          />
        </div>

        <FunctionReference />

        <EvaluationPreview
          view={view}
          setView={setView}
          expression={current?.valid ? checked.expression : undefined}
          unit={checked.unit || undefined}
        />

        {save.isError && (
          <Alert tone="error">
            {t('derived.editor.saveError', { message: save.error.message })}
          </Alert>
        )}

        <div className="tr-derived-buttons">
          <Button
            variant="ghost"
            onClick={() => (dirty ? setDiscarding(true) : close())}
            disabled={save.isPending}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            variant="primary"
            disabled={!canSave}
            loading={save.isPending}
          >
            {t('derived.editor.save')}
          </Button>
        </div>
      </form>

      <ConfirmDialog
        open={discarding}
        title={t('derived.editor.discardTitle')}
        confirmLabel={t('derived.editor.discard')}
        onConfirm={() => {
          setDiscarding(false)
          close()
        }}
        onClose={() => setDiscarding(false)}
        destructive
      >
        {t('derived.editor.discardText')}
      </ConfirmDialog>
    </Section>
  )
}

/** What the server says about the expression on screen. */
function CheckStatus({
  id,
  empty,
  checking,
  error,
  validation,
  problems,
  expression,
}: {
  id: string
  empty: boolean
  checking: boolean
  error: Error | null
  validation: Validation | undefined
  problems: readonly DslProblem[]
  expression: string
}) {
  const { t } = useTranslation()
  if (empty) return null
  return (
    <output id={id} className="tr-derived-check">
      {error ? (
        <Alert tone="error">
          {t('derived.editor.checkError', { message: error.message })}
        </Alert>
      ) : checking || !validation ? (
        <p className="ui-text ui-secondary ui-inline-row">
          <LoaderCircle size={14} className="ui-spinner" aria-hidden />
          {t('derived.editor.checking')}
        </p>
      ) : !validation.valid ? (
        <div>
          <p className="ui-text tr-derived-error ui-inline-row">
            <CircleAlert size={14} aria-hidden />
            {t('derived.editor.problems', { count: problems.length })}
          </p>
          <ul className="tr-derived-problems">
            {problems.map((problem, i) => {
              const at = expression.slice(problem.from, problem.to)
              return (
                <li key={i}>
                  {problem.message}
                  {at && (
                    <>
                      {' '}
                      <code className="tr-derived-at">{at}</code>
                    </>
                  )}
                </li>
              )
            })}
          </ul>
        </div>
      ) : (
        <div className="ui-stack" style={{ gap: 'var(--space-2)' }}>
          <p className="ui-text tr-derived-ok ui-inline-row">
            <CircleCheck size={14} aria-hidden />
            {t('derived.editor.valid')}
          </p>
          <ValidationFacts validation={validation} />
        </div>
      )}
    </output>
  )
}

const code = (text: string) => <code className="ui-code">{text}</code>

function ValidationFacts({ validation }: { validation: Validation }) {
  const { t } = useTranslation()
  const list = (items: readonly string[]) =>
    items.length === 0 ? t('common.none') : code(items.join(', '))
  return (
    <Facts
      items={[
        {
          label: t('derived.editor.kind'),
          value:
            validation.kind === 'series'
              ? t('derived.editor.kindSeries', {
                  dimension: validation.x_dimension ?? '',
                  unit: validation.x_unit || t('metrics.unit.dimensionless'),
                })
              : t('derived.editor.kindScalar'),
        },
        {
          label: t('derived.editor.resultUnit'),
          value: validation.unit
            ? code(validation.unit)
            : t('metrics.unit.dimensionless'),
        },
        {
          label: t('derived.editor.inputMetrics'),
          value: list(validation.inputs.metrics),
        },
        {
          label: t('derived.editor.inputSeries'),
          value: list(validation.inputs.series),
        },
        {
          label: t('derived.editor.uses'),
          value:
            validation.uses.length === 0
              ? t('derived.detail.usesNone')
              : code(validation.uses.join(', ')),
        },
      ]}
    />
  )
}
