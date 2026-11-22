import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type {
  DerivedMetric,
  DerivedMetricVersion,
  EvaluateResponse,
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

const version = (
  name: string,
  n: number,
  overrides: Partial<DerivedMetricVersion> = {},
): DerivedMetricVersion => ({
  id: n * 100 + name.length,
  name,
  version: n,
  expression: 'bytes_transferred / load_time',
  unit: 'MB/s',
  description: 'Average download rate.',
  created_at: '2026-11-01T08:00:00Z',
  uses: [],
  ...overrides,
})

function derivedMetrics(): DerivedMetric[] {
  return [
    {
      id: 1,
      name: 'throughput',
      versions: [
        version('throughput', 1),
        version('throughput', 2, {
          unit: 'Mbit/s',
          description: 'Comparable to the link bandwidth.',
        }),
      ],
    },
    {
      id: 2,
      name: 'efficiency',
      versions: [
        version('efficiency', 1, {
          expression: 'derived.throughput / power',
          unit: 'MB/J',
          uses: [{ name: 'throughput', version: 1, version_id: 110 }],
        }),
      ],
    },
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
    expect(detail.getAllByText('bytes_transferred / load_time')).not.toHaveLength(0)
    expect(detail.getByText('Mbit/s')).toBeInTheDocument()
    expect(detail.getByText('derived.throughput')).toBeInTheDocument()
    expect(detail.getByText('Comparable to the link bandwidth.')).toBeInTheDocument()
    // efficiency still pins version 1 of throughput.
    expect(
      detail.getByRole('link', { name: 'efficiency v1' }),
    ).toHaveAttribute('href', '/derived-metrics?name=efficiency')
    expect(detail.getByText('pins v1')).toBeInTheDocument()

    const history = within(
      detail.getByRole('list', { name: 'Versions of throughput' }),
    )
    expect(history.getAllByRole('link').map((l) => l.getAttribute('href'))).toEqual([
      '/derived-metrics?name=throughput',
      '/derived-metrics?name=throughput&version=1',
    ])
  })

  it('shows an older version and the versions a metric uses', async () => {
    mockApi({ [listPath]: derivedMetrics })
    const router = renderApp('/derived-metrics?name=throughput&version=1')

    const detail = within(await panel('throughput'))
    expect(
      detail.getByText('This is version 1; the latest is version 2.'),
    ).toBeInTheDocument()
    expect(detail.getByText('MB/s')).toBeInTheDocument()

    const user = userEvent.setup()
    const list = within(
      screen.getByRole('navigation', { name: 'Derived metrics' }),
    )
    await user.click(list.getByRole('link', { name: /^efficiency/ }))
    const efficiency = within(await panel('efficiency'))
    expect(
      efficiency.getByRole('link', { name: 'throughput v1' }),
    ).toHaveAttribute('href', '/derived-metrics?name=throughput&version=1')
    expect(efficiency.getByText('latest is v2')).toBeInTheDocument()
    expect(efficiency.getByText('No other derived metric uses it.')).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=efficiency')
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
        const body = (await request.json()) as {
          expression: string
          unit: string
          description: string
        }
        saved.push({ method: request.method, body })
        const created = version('rate', 1, { ...body })
        metrics = [...metrics, { id: 3, name: 'rate', versions: [created] }]
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
    expect(await screen.findByText('Saved as version 1.')).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=rate')
  })

  it('starts a new version from the selected one', async () => {
    const saved: unknown[] = []
    let metrics = derivedMetrics()
    mockApi({
      [listPath]: () => metrics,
      [validatePath]: validate,
      [`${listPath}/throughput`]: async (_, request) => {
        const body = (await request.json()) as Partial<DerivedMetricVersion>
        saved.push(body)
        const created = version('throughput', 3, body)
        metrics = metrics.map((m) =>
          m.name === 'throughput'
            ? { ...m, versions: [...m.versions, created] }
            : m,
        )
        return { ...created, created: true }
      },
    })
    const router = renderApp('/derived-metrics?name=throughput&version=1')

    const user = userEvent.setup()
    await user.click(await screen.findByRole('link', { name: 'New version' }))
    expect(currentQuery(router)).toBe('name=throughput&version=1&edit=version')

    const form = within(await panel('New version of throughput (v3)'))
    expect(
      form.getByText(/Starts from version 1; the latest is version 2/),
    ).toBeInTheDocument()
    expect(await editor()).toHaveValue('bytes_transferred / load_time')
    expect(form.getByRole('combobox', { name: 'Unit' })).toHaveValue('MB/s')
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    // Saving an older version unchanged makes it the latest again.
    expect(form.getByRole('button', { name: 'Save version 3' })).toBeEnabled()

    await user.click(form.getByRole('button', { name: 'Save version 3' }))
    await waitFor(() => expect(saved).toHaveLength(1))
    expect(saved[0]).toEqual({
      expression: 'bytes_transferred / load_time',
      unit: 'MB/s',
      description: 'Average download rate.',
    })
    expect(await screen.findByText('Saved as version 3.')).toBeInTheDocument()
    expect(currentQuery(router)).toBe('name=throughput')
  })

  it('asks before discarding changes and saves nothing unchanged', async () => {
    mockApi({ [listPath]: derivedMetrics, [validatePath]: validate })
    const router = renderApp('/derived-metrics?name=throughput&edit=version')

    const form = within(await panel('New version of throughput (v3)'))
    expect(await form.findByText('The expression is valid.')).toBeInTheDocument()
    expect(form.getByRole('button', { name: 'Save version 3' })).toBeDisabled()

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
