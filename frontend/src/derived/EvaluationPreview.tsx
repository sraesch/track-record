import type { TFunction } from 'i18next'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { CaseExecutionSummary, RunSummary } from '../api/client'
import { formatDateTime, formatJsonValue } from '../format'
import { useCaseExecutions, useRun, useRunList } from '../runs/queries'
import { Alert, Combobox, ProgressBar, Surface } from '../ui'
import {
  type DerivedView,
  resultsBySample,
  type SetDerivedView,
} from './derivedModel'
import { ScalarTable, SeriesResultChart } from './EvaluationViews'
import { useEvaluation } from './queries'
import './derived.css'

/** Points per series result; about one per pixel of a wide chart. */
export const previewPoints = 1000

// Newest runs first, invalid ones included: an expression may be tried on
// any run.
const recentRuns = { validity: 'all' } as const

type Props = {
  view: DerivedView
  setView: SetDerivedView
  /** The checked, valid expression; undefined while there is none. */
  expression: string | undefined
  unit: string | undefined
}

/**
 * Tries the expression on the samples of one case execution before it is
 * saved. Run and case execution are in the URL, so the preview survives
 * a reload and can be shared (FR-21).
 */
export function EvaluationPreview({ view, setView, expression, unit }: Props) {
  const { t } = useTranslation()
  const runs = useRunList(recentRuns)
  const chosenRun = useRun(view.run ?? '')
  const cases = useCaseExecutions(view.run ?? '', false, false)

  const runOptions = useMemo(() => {
    const list = runs.data?.pages[0]?.runs ?? []
    const chosen = chosenRun.data
    return chosen && !list.some((r) => r.id === chosen.id)
      ? [chosen, ...list]
      : list
  }, [runs.data, chosenRun.data])
  const run = runOptions.find((r) => r.id === view.run) ?? null
  const caseOptions = cases.data ?? []
  const caseExecution =
    caseOptions.find((c) => c.id === view.caseExecution) ?? null

  const request =
    expression && caseExecution
      ? {
          case_execution_id: caseExecution.id,
          expression,
          unit: unit ?? null,
          max_points: previewPoints,
        }
      : undefined
  const evaluation = useEvaluation(request)
  const response = request ? evaluation.data : undefined

  const runLabel = (r: RunSummary) =>
    t('derived.preview.runOption', {
      suite: r.suite.name,
      started: formatDateTime(r.started_at),
      branch: r.sut_version.branch ?? r.sut_version.commit.slice(0, 8),
    })

  const note = (text: string) => <p className="ui-text ui-secondary">{text}</p>

  return (
    <Surface className="tr-derived-preview">
      <h3 className="body-medium ui-heading">{t('derived.preview.title')}</h3>
      <p className="ui-text ui-secondary">{t('derived.preview.help')}</p>
      <div className="tr-derived-pickers">
        <Combobox
          label={t('derived.preview.run')}
          options={runOptions}
          loading={runs.isPending}
          value={run}
          getOptionLabel={runLabel}
          getOptionKey={(r) => r.id}
          onChange={(next) =>
            setView({ ...view, run: next?.id, caseExecution: undefined })
          }
        />
        <Combobox
          label={t('derived.preview.caseExecution')}
          options={caseOptions}
          loading={cases.isFetching}
          disabled={!view.run}
          value={caseExecution}
          getOptionLabel={(c) => caseLabel(c, t)}
          getOptionKey={(c) => c.id}
          onChange={(next) => setView({ ...view, caseExecution: next?.id })}
        />
      </div>

      <div className="tr-derived-progress">
        {request && evaluation.isFetching && <ProgressBar />}
      </div>

      {!caseExecution ? (
        note(t('derived.preview.choose'))
      ) : !expression ? (
        note(t('derived.preview.needsValid'))
      ) : evaluation.isError ? (
        <Alert tone="error">
          {t('derived.preview.error', { message: evaluation.error.message })}
        </Alert>
      ) : !response ? null : response.samples.length === 0 ? (
        note(t('derived.preview.noSamples'))
      ) : response.kind === 'series' ? (
        <SeriesResultChart
          title={t('derived.preview.result')}
          response={response}
        />
      ) : (
        <ScalarTable
          framed
          label={t('derived.preview.result')}
          samples={response.samples.map((s) => ({
            id: s.sample_id,
            repetition: s.repetition,
            is_warmup: s.is_warmup,
          }))}
          columns={[
            {
              key: 'result',
              label: t('derived.preview.value'),
              unit: response.unit,
              results: resultsBySample(response),
            },
          ]}
        />
      )}
    </Surface>
  )
}

/** A case execution as the picker names it: scenario, device, parameters. */
function caseLabel(c: CaseExecutionSummary, t: TFunction): string {
  const parameters = Object.entries(c.test_case.resolved_parameters)
    .map(([name, value]) => `${name}=${formatJsonValue(value)}`)
    .join(', ')
  return t('derived.preview.caseOption', {
    scenario: c.test_case.scenario,
    version: c.test_case.scenario_version,
    device: c.client_device.resolved_name,
    parameters: parameters || t('common.none'),
  })
}
