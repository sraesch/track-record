import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type {
  DerivedMetric,
  EvaluateRequest,
  EvaluateResponse,
  SampleResult,
} from '../api/client'
import {
  caseExecutionDetail,
  caseExecutionPath,
  runDetail,
  runId,
  runPath,
} from '../runs/detail/fixtures'
import { mockApi, problem } from '../test/api'
import { renderApp } from '../test/render'

const listPath = '/ui/api/v1/derived-metrics'
const evaluatePath = '/ui/api/v1/derived-metrics/evaluate'

const metric = (name: string, expression: string, unit = ''): DerivedMetric => ({
  id: name.length,
  name,
  versions: [
    {
      id: name.length * 10,
      name,
      version: 1,
      expression,
      unit,
      description: '',
      created_at: '2026-11-01T08:00:00Z',
      uses: [],
    },
  ],
})

const metrics = [
  metric('throughput', 'bytes / load_time', 'MB/s'),
  metric('smooth_fps', 'moving_avg(series.fps, 1[s])', 'fps'),
  metric('stall', 'stalls / load_time', '1/s'),
  metric('broken', 'time_to(series.never_uploaded, 1)', 's'),
]

/** One result per sample of the fixture (s0 warmup, s1, s2). */
const samples = (
  result: (repetition: number) => Partial<SampleResult>,
): SampleResult[] =>
  [0, 1, 2].map((repetition) => ({
    sample_id: `s${repetition}`,
    repetition,
    is_warmup: repetition === 0,
    status: 'value',
    ...result(repetition),
  }))

const responses: Record<string, EvaluateResponse | Response> = {
  throughput: {
    expression: 'derived.throughput@1',
    kind: 'scalar',
    unit: 'MB/s',
    samples: samples((r) =>
      r === 2 ? { status: 'no_value' } : { value: 1.5 + r },
    ),
  },
  smooth_fps: {
    expression: 'derived.smooth_fps@1',
    kind: 'series',
    unit: 'fps',
    x_dimension: 'time',
    x_unit: 's',
    samples: samples((r) =>
      r === 1
        ? { status: 'error', message: 'x decreases' }
        : { series: { x: [0, 1, 2], y: [30, 58, 60] } },
    ),
  },
  stall: {
    expression: 'derived.stall@1',
    kind: 'scalar',
    unit: '1/s',
    samples: samples(() => ({
      status: 'missing',
      message: 'sample has no stalls',
    })),
  },
  broken: problem(422, 'invalid_expression', 'series never_uploaded has no unit'),
}

describe('Derived metrics of a case execution', () => {
  it('evaluates every derived metric on the samples', async () => {
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
      [listPath]: () => metrics,
      [evaluatePath]: async (_, request) => {
        const body = (await request.json()) as EvaluateRequest
        expect(body).toMatchObject({ case_execution_id: 'ce1', max_points: 1000 })
        return responses[body.name ?? '']
      },
    })
    renderApp(`/runs/${runId}/cases/ce1`)

    const section = within(
      await screen.findByRole('region', { name: 'Derived metrics' }),
    )
    const table = within(
      await section.findByRole('table', { name: 'Derived metrics' }),
    )
    expect(table.getByRole('link', { name: 'throughput' })).toHaveAttribute(
      'href',
      '/derived-metrics?name=throughput',
    )
    const rows = table.getAllByRole('row').map((row) =>
      within(row)
        .queryAllByRole('cell')
        .map((cell) => cell.textContent),
    )
    expect(rows.slice(1)).toEqual([
      ['0Warmup', '1.5'],
      ['1', '2.5'],
      ['2', 'no value'],
    ])

    // The series result as a chart, the sample without a series named.
    expect(await section.findByText('smooth_fps')).toBeInTheDocument()
    expect(await section.findByTestId('chart')).toHaveTextContent(
      'Repetition 0 (warmup): 3 points',
    )
    expect(section.getByText('Repetition 1:')).toBeInTheDocument()
    expect(section.getByText('error')).toBeInTheDocument()

    // Inputs this case does not have, or a definition that does not fit.
    expect(
      await section.findByText('Not applicable here (inputs missing):', {
        exact: false,
      }),
    ).toBeInTheDocument()
    expect(section.getByRole('link', { name: 'stall' })).toHaveAccessibleDescription(
      'sample has no stalls',
    )
    expect(section.getByRole('link', { name: 'broken' })).toHaveAccessibleDescription(
      'series never_uploaded has no unit',
    )
  })

  it('points to the derived metrics page when there are none', async () => {
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
    })
    renderApp(`/runs/${runId}/cases/ce1`)

    const section = within(
      await screen.findByRole('region', { name: 'Derived metrics' }),
    )
    expect(
      await section.findByRole('link', { name: 'Define one' }),
    ).toHaveAttribute('href', '/derived-metrics?edit=new')
  })
})
