import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type {
  DerivedMetric,
  EvaluateResponse,
  SaveDerivedMetric,
  Validation,
} from '../api/client'
import {
  caseExecution,
  casesPath,
  runDetail,
  runId,
  runPath,
} from '../runs/detail/fixtures'
import { mockApi } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

const listPath = '/ui/api/v1/derived-metrics'
const validatePath = '/ui/api/v1/derived-metrics/validate'
const evaluatePath = '/ui/api/v1/derived-metrics/evaluate'

const metric = (
  name: string,
  overrides: Partial<DerivedMetric> = {},
): DerivedMetric => ({
  id: name.length,
  name,
  expression: 'bytes_transferred / load_time',
  unit: 'Mbit/s',
  description: 'Comparable to the link bandwidth.',
  created_at: '2026-11-01T08:00:00Z',
  updated_at: '2026-11-03T08:00:00Z',
  uses: [],
  ...overrides,
})

function derivedMetrics(): DerivedMetric[] {
  return [
    metric('throughput'),
    metric('efficiency', {
      expression: 'derived.throughput / power',
      unit: 'MB/J',
      description: '',
      uses: ['throughput'],
    }),
  ]
}

const valid = (overrides: Partial<Validation> = {}): Validation => ({
  valid: true,
  diagnostics: [],
  kind: 'scalar',
  unit: 'B/s',
  inferred_unit: 'B/s',
  uses: [],
  inputs: { metrics: ['bytes_transferred', 'load_time'], series: [] },
  ...overrides,
})

const invalid = (start: number, end: number, message: string): Validation => ({
  valid: false,
  diagnostics: [{ start, end, message }],
  uses: [],
  inputs: { metrics: [], series: [] },
})

/** Answers validations: an expression ending in an operator is invalid. */
async function validate(_: URL, request: Request) {
  const body = (await request.json()) as { expression: string }
  const text = body.expression.trimEnd()
  return /[-+*/]$/.test(text)
    ? invalid(text.length - 1, text.length, 'expected an operand')
    : valid()
}

const editor = () => screen.findByRole('textbox', { name: 'Expression' })
const panel = (name: string) => screen.findByRole('region', { name })

describe('Derived metrics', () => {
  it('lists the derived metrics and shows the selected one', async () => {
    mockApi({ [listPath]: derivedMetrics })
    renderApp('/derived-metrics?name=throughput')

    const list = within(
      await screen.findByRole('navigation', { name: 'Derived metrics' }),
    )
    expect(await list.findByRole('link', { name: /^efficiency/ })).toHaveAttribute(
      'href',
      '/derived-metrics?name=efficiency',
    )
    expect(list.getByRole('link', { name: /^throughput/ })).toHaveAttribute(
      'aria-current',
      'page',
    )
    expect(list.getByText('2 derived metrics')).toBeInTheDocument()

    const detail = within(await panel('throughput'))
    expect(detail.getByText('bytes_transferred / load_time')).toBeInTheDocument()
    expect(detail.getByText('Mbit/s')).toBeInTheDocument()
    expect(detail.getByText('derived.throughput')).toBeInTheDocument()
    expect(detail.getByText('Comparable to the link bandwidth.')).toBeInTheDocument()
    expect(detail.getByRole('link', { name: 'efficiency' })).toHaveAttribute(
      'href',
      '/derived-metrics?name=efficiency',
    )
    // One definition, which "Edit" changes: there are no versions.
    expect(detail.getByRole('link', { name: 'Edit' })).toHaveAttribute(
      'href',
      '/derived-metrics?name=throughput&edit=change',
    )
    expect(detail.queryByText(/version/i)).toBeNull()
  })

  it('links to the derived metrics a metric uses', async () => {
    mockApi({ [listPath]: derivedMetrics })
    const router = renderApp('/derived-metrics?name=efficiency')

    const efficiency = within(await panel('efficiency'))
    expect(efficiency.getByText('No other derived metric uses it.')).toBeInTheDocument()
    const user = userEvent.setup()
    await user.click(efficiency.getByRole('link', { name: 'throughput' }))
    expect(await panel('throughput')).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=throughput')
  })

  it('searches the list and says when a metric does not exist', async () => {
    mockApi({ [listPath]: derivedMetrics })
    const router = renderApp('/derived-metrics?name=nope')

    expect(
      await screen.findByText('There is no derived metric nope.'),
    ).toBeInTheDocument()

    const user = userEvent.setup()
    await enterText(
      user,
      screen.getByRole('searchbox', { name: 'Search derived metrics' }),
      'power',
    )
    const list = within(screen.getByRole('navigation', { name: 'Derived metrics' }))
    await waitFor(() =>
      expect(list.queryByRole('link', { name: /^throughput/ })).toBeNull(),
    )
    expect(list.getByRole('link', { name: /^efficiency/ })).toBeInTheDocument()
    expect(currentQuery(router)).toBe('q=power&name=nope')
  })

  it('writes a new derived metric with the server checking it', async () => {
    const saved: unknown[] = []
    const checks: unknown[] = []
    let metrics = derivedMetrics()
    mockApi({
      [listPath]: () => metrics,
      [validatePath]: (url, request) => {
        checks.push(request.clone())
        return validate(url, request)
      },
      [`${listPath}/rate`]: async (_, request) => {
        const body = (await request.json()) as SaveDerivedMetric
        saved.push({ method: request.method, body })
        const created = metric('rate', body)
        metrics = [...metrics, created]
        return { ...created, created: true }
      },
    })
    const router = renderApp('/derived-metrics')

    const user = userEvent.setup()
    await user.click(await screen.findByRole('link', { name: 'New' }))
    expect(currentQuery(router)).toBe('edit=new')
    const form = within(await panel('New derived metric'))

    await enterText(user, form.getByRole('textbox', { name: 'Name' }), 'rate')
    await enterText(user, await editor(), 'bytes_transferred /')
    expect(await form.findByText('1 problem')).toBeInTheDocument()
    expect(form.getByText('expected an operand')).toBeInTheDocument()
    expect(await editor()).toHaveAttribute('aria-invalid', 'true')
    expect(form.getByRole('button', { name: 'Save' })).toBeDisabled()

    await enterText(user, await editor(), ' load_time')
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    expect(form.getByText('bytes_transferred, load_time')).toBeInTheDocument()
    expect(
      form.getByText("Leave empty for the expression's unit: B/s."),
    ).toBeInTheDocument()
    // Checked with the name it will be saved under, which finds cycles.
    expect(await (checks.at(-1) as Request).json()).toEqual({
      expression: 'bytes_transferred / load_time',
      unit: null,
      name: 'rate',
    })

    await enterText(
      user,
      form.getByRole('textbox', { name: 'Description' }),
      'Bytes per second.',
    )
    await user.click(form.getByRole('button', { name: 'Save' }))

    await waitFor(() => expect(saved).toHaveLength(1))
    expect(saved[0]).toEqual({
      method: 'PUT',
      body: {
        expression: 'bytes_transferred / load_time',
        // An empty unit saves the expression's own unit.
        unit: 'B/s',
        description: 'Bytes per second.',
      },
    })
    expect(
      await screen.findByText('The derived metric is created.'),
    ).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=rate')
  })

  it('does not replace an existing metric from "New"', async () => {
    mockApi({ [listPath]: derivedMetrics, [validatePath]: validate })
    renderApp('/derived-metrics?edit=new')

    const user = userEvent.setup()
    const form = within(await panel('New derived metric'))
    await enterText(user, form.getByRole('textbox', { name: 'Name' }), 'throughput')
    await enterText(user, await editor(), 'load_time')
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    expect(
      form.getByText(/A derived metric with this name exists/),
    ).toBeInTheDocument()
    expect(form.getByRole('button', { name: 'Save' })).toBeDisabled()
  })

  it('changes a derived metric in place', async () => {
    const saved: unknown[] = []
    const checks: unknown[] = []
    let metrics = derivedMetrics()
    mockApi({
      [listPath]: () => metrics,
      [validatePath]: async (url, request) => {
        checks.push(await request.clone().json())
        return validate(url, request)
      },
      [`${listPath}/throughput`]: async (_, request) => {
        const body = (await request.json()) as SaveDerivedMetric
        saved.push(body)
        const changed = metric('throughput', {
          ...body,
          updated_at: '2026-11-22T08:00:00Z',
        })
        metrics = metrics.map((m) => (m.name === 'throughput' ? changed : m))
        return { ...changed, created: false }
      },
    })
    const router = renderApp('/derived-metrics?name=throughput')

    const user = userEvent.setup()
    await user.click(await screen.findByRole('link', { name: 'Edit' }))
    expect(currentQuery(router)).toBe('name=throughput&edit=change')

    const form = within(await panel('Change throughput'))
    // The change reaches every analysis and the metrics built on this one.
    expect(form.getByRole('alert')).toHaveTextContent(
      /no versions.*including past runs\. It also changes 1 derived metric built on it: efficiency\./,
    )
    expect(await editor()).toHaveValue('bytes_transferred / load_time')
    expect(form.getByRole('combobox', { name: 'Unit' })).toHaveValue('Mbit/s')
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    expect(form.getByRole('button', { name: 'Save' })).toBeDisabled()

    await enterText(user, await editor(), ' * 8')
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    // Checked under its name: the server also checks the metrics built on it.
    expect(checks.at(-1)).toEqual({
      expression: 'bytes_transferred / load_time * 8',
      unit: 'Mbit/s',
      name: 'throughput',
    })
    await user.click(form.getByRole('button', { name: 'Save' }))

    await waitFor(() => expect(saved).toHaveLength(1))
    expect(saved[0]).toEqual({
      expression: 'bytes_transferred / load_time * 8',
      unit: 'Mbit/s',
      description: 'Comparable to the link bandwidth.',
    })
    expect(
      await screen.findByText(
        'The change is saved. It applies to every analysis, including past runs.',
      ),
    ).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=throughput')
    const detail = within(await panel('throughput'))
    expect(
      await detail.findByText('bytes_transferred / load_time * 8'),
    ).toBeInTheDocument()
  })

  it('asks before discarding changes and saves nothing unchanged', async () => {
    mockApi({ [listPath]: derivedMetrics, [validatePath]: validate })
    const router = renderApp('/derived-metrics?name=throughput&edit=change')

    const form = within(await panel('Change throughput'))
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    expect(form.getByRole('button', { name: 'Save' })).toBeDisabled()

    const user = userEvent.setup()
    await enterText(user, await editor(), ' * 8')
    await user.click(form.getByRole('button', { name: 'Cancel' }))
    const dialog = within(
      await screen.findByRole('dialog', { name: 'Discard your changes?' }),
    )
    await user.click(dialog.getByRole('button', { name: 'Discard' }))
    expect(await panel('throughput')).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=throughput')
  })

  it('tries the expression on a case execution', async () => {
    const response: EvaluateResponse = {
      expression: 'bytes_transferred / load_time',
      kind: 'scalar',
      unit: 'B/s',
      samples: [
        { sample_id: 's0', repetition: 0, is_warmup: true, status: 'value', value: 1234567.891 },
        { sample_id: 's1', repetition: 1, is_warmup: false, status: 'no_value' },
        {
          sample_id: 's2',
          repetition: 2,
          is_warmup: false,
          status: 'missing',
          message: 'sample has no load_time',
        },
      ],
    }
    const evaluated: unknown[] = []
    mockApi({
      [listPath]: derivedMetrics,
      [validatePath]: validate,
      '/ui/api/v1/runs': () => ({ runs: [runDetail()], next_cursor: null }),
      [runPath]: () => runDetail(),
      [casesPath]: () => [caseExecution('ce1')],
      [evaluatePath]: async (_, request) => {
        evaluated.push(await request.json())
        return response
      },
    })
    const router = renderApp('/derived-metrics?edit=new')

    const user = userEvent.setup()
    await enterText(user, await editor(), 'bytes_transferred / load_time')
    const preview = within(await panel('New derived metric'))
    expect(
      preview.getByText('Choose a run and a case execution to see the values.'),
    ).toBeInTheDocument()

    await user.click(preview.getByRole('combobox', { name: 'Run' }))
    await user.click(await screen.findByRole('option', { name: /^streaming-nightly/ }))
    await user.click(preview.getByRole('combobox', { name: 'Case execution' }))
    await user.click(await screen.findByRole('option', { name: /^orbit v2/ }))
    expect(currentQuery(router)).toBe(`edit=new&run=${runId}&case=ce1`)

    const table = within(await screen.findByRole('table', { name: 'Result' }))
    const cells = table.getAllByRole('row').slice(1).map((row) =>
      within(row).getAllByRole('cell').map((cell) => cell.textContent),
    )
    expect(cells).toEqual([
      ['0Warmup', '1,234,570'],
      ['1', 'no value'],
      ['2', 'missing'],
    ])
    expect(table.getByText('1,234,570')).toHaveAttribute('title', '1,234,567.891')
    expect(evaluated.at(-1)).toEqual({
      case_execution_id: 'ce1',
      expression: 'bytes_transferred / load_time',
      unit: null,
      max_points: 1000,
    })
  })
})
