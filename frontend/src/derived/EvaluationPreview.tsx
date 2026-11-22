import Alert from '@mui/material/Alert'
import Autocomplete from '@mui/material/Autocomplete'
import Box from '@mui/material/Box'
import LinearProgress from '@mui/material/LinearProgress'
import Paper from '@mui/material/Paper'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import type { TFunction } from 'i18next'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { CaseExecutionSummary, RunSummary } from '../api/client'
import { formatDateTime, formatJsonValue } from '../format'
import { useCaseExecutions, useRun, useRunList } from '../runs/queries'
import {
  type DerivedView,
  resultsBySample,
  type SetDerivedView,
} from './derivedModel'
import { ScalarTable, SeriesResultChart } from './EvaluationViews'
import { useEvaluation } from './queries'

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

  return (
    <Paper variant="outlined" sx={{ p: 2, minWidth: 0 }}>
      <Typography variant="subtitle2" component="h3">
        {t('derived.preview.title')}
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
        {t('derived.preview.help')}
      </Typography>
      <Box
        sx={{
          display: 'grid',
          gap: 2,
          gridTemplateColumns: { xs: '1fr', sm: '1fr 1fr' },
          mb: 1.5,
        }}
      >
        <Autocomplete
          size="small"
          options={runOptions}
          loading={runs.isPending}
          value={run}
          getOptionLabel={runLabel}
          isOptionEqualToValue={(a, b) => a.id === b.id}
          onChange={(_, next) =>
            setView({ ...view, run: next?.id, caseExecution: undefined })
          }
          renderInput={(params) => (
            <TextField {...params} label={t('derived.preview.run')} />
          )}
        />
        <Autocomplete
          size="small"
          options={caseOptions}
          loading={cases.isFetching}
          disabled={!view.run}
          value={caseExecution}
          getOptionLabel={(c) => caseLabel(c, t)}
          isOptionEqualToValue={(a, b) => a.id === b.id}
          onChange={(_, next) =>
            setView({ ...view, caseExecution: next?.id })
          }
          renderInput={(params) => (
            <TextField {...params} label={t('derived.preview.caseExecution')} />
          )}
        />
      </Box>

      <Box sx={{ height: 4, mb: 1 }}>
        {request && evaluation.isFetching && (
          <LinearProgress aria-label={t('common.loading')} />
        )}
      </Box>

      {!caseExecution ? (
        <Typography variant="body2" color="text.secondary">
          {t('derived.preview.choose')}
        </Typography>
      ) : !expression ? (
        <Typography variant="body2" color="text.secondary">
          {t('derived.preview.needsValid')}
        </Typography>
      ) : evaluation.isError ? (
        <Alert severity="error">
          {t('derived.preview.error', { message: evaluation.error.message })}
        </Alert>
      ) : !response ? null : response.samples.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {t('derived.preview.noSamples')}
        </Typography>
      ) : response.kind === 'series' ? (
        <SeriesResultChart title={t('derived.preview.result')} response={response} />
      ) : (
        <Box sx={{ mx: -2 }}>
          <ScalarTable
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
        </Box>
      )}
    </Paper>
  )
}

/** A case execution as the picker names it: scenario, device, parameters. */
function caseLabel(
  c: CaseExecutionSummary,
  t: TFunction,
): string {
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
