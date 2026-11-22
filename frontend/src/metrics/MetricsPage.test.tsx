import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { Metric, Unit } from '../api/client'
import { mockApi, problem } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

const metric = (
  id: number,
  name: string,
  fields: Partial<Metric> = {},
): Metric => ({
  id,
  name,
  unit: 's',
  value_type: 'float',
  direction: 'lower_is_better',
  description: '',
  created_at: '2026-10-01T08:00:00Z',
  updated_at: '2026-11-01T08:00:00Z',
  has_results: true,
  used_by: [],
  ...fields,
})

const metrics: Metric[] = [
  metric(1, 'load_time', {
    description: 'Until the first frame.',
    used_by: ['padded'],
  }),
  metric(2, 'fps', { unit: 'fps', direction: 'higher_is_better' }),
  metric(3, 'triangles', {
    unit: 'tris',
    value_type: 'integer',
    description: 'Triangles drawn per frame.',
    has_results: false,
  }),
]

const units: Unit[] = [
  { symbol: 'ms', quantity: 'time', to_base: 1e-3, name: 'millisecond' },
  { symbol: 's', quantity: 'time', to_base: 1, name: 'second' },
  { symbol: 'min', quantity: 'time', to_base: 60, name: 'minute' },
  {
    symbol: 'fps',
    quantity: 'frame rate',
    to_base: 1,
    name: 'frames per second',
  },
]

function setup(handlers: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    '/ui/api/v1/metrics': () => metrics,
    '/ui/api/v1/units': () => units,
    ...handlers,
  })
}

const table = () => screen.findByRole('table', { name: 'Metric definitions' })
const bodyRows = async () =>
  within(await table()).getAllByRole('row').slice(1)
const dialog = (name: string) => screen.findByRole('dialog', { name })
/** The body of the recorded request with `method`. */
const sent = async (requests: Request[], method: string) =>
  requests.find((r) => r.method === method)?.json()

describe('Metrics page', () => {
  it('lists the metrics with their definition', async () => {
    setup()
    renderApp('/metrics')

    const rows = await bodyRows()
    expect(
      rows.map((r) => within(r).getAllByRole('cell')[0]!.textContent),
    ).toEqual(['fps', 'load_time', 'triangles'])
    const load = within(rows[1]!)
    expect(load.getByText('s')).toBeInTheDocument()
    expect(load.getByText('Decimal')).toBeInTheDocument()
    expect(load.getByText('Lower is better')).toBeInTheDocument()
    expect(load.getByText('Until the first frame.')).toBeInTheDocument()
    expect(within(rows[0]!).getByText('Higher is better')).toBeInTheDocument()
    expect(within(rows[2]!).getByText('Integer')).toBeInTheDocument()
    expect(screen.getByText('3 metrics')).toBeInTheDocument()
  })

  it('marks units that are not in the catalogue', async () => {
    setup()
    renderApp('/metrics')
    const rows = await bodyRows()
    expect(await within(rows[2]!).findByText('Custom')).toBeInTheDocument()
    expect(within(rows[1]!).queryByText('Custom')).not.toBeInTheDocument()
  })

  it('changes the definition of a metric', async () => {
    const requests: Request[] = []
    setup({
      '/ui/api/v1/metrics/load_time': (_, request) => {
        requests.push(request.clone())
        return metric(1, 'load_time', { unit: 'ms' })
      },
    })
    const user = userEvent.setup()
    renderApp('/metrics')
    await table()

    await user.click(
      screen.getByRole('button', { name: 'Edit metric load_time' }),
    )
    const edit = within(await dialog('Edit load_time'))
    expect(edit.getByText(/applies to all its results/)).toBeInTheDocument()
    // Nothing is reinterpreted before the unit changes.
    expect(edit.queryByText(/not converted: they are read/)).toBeNull()

    const unit = edit.getByRole('combobox', { name: 'Unit' })
    await user.clear(unit)
    await enterText(user, unit, 'ms')
    expect(
      edit.getByText(/not converted: they are read in the new unit/),
    ).toBeInTheDocument()
    expect(edit.getByText('padded')).toBeInTheDocument()

    await user.click(edit.getByRole('button', { name: 'Save' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(await sent(requests, 'PUT')).toEqual({
      unit: 'ms',
      value_type: 'float',
      direction: 'lower_is_better',
      description: 'Until the first frame.',
    })
  })

  it('shows why a change was rejected', async () => {
    setup({
      '/ui/api/v1/metrics/load_time': () =>
        problem(
          422,
          'breaks_derived_metrics',
          'derived.padded uses load_time and would no longer be valid',
        ),
    })
    const user = userEvent.setup()
    renderApp('/metrics')
    await table()

    await user.click(
      screen.getByRole('button', { name: 'Edit metric load_time' }),
    )
    const edit = within(await dialog('Edit load_time'))
    await user.click(edit.getByRole('button', { name: 'Save' }))
    expect(
      await edit.findByText(/derived.padded uses load_time/),
    ).toBeInTheDocument()
  })

  it('renames a metric', async () => {
    const requests: Request[] = []
    setup({
      '/ui/api/v1/metrics/load_time/rename': (_, request) => {
        requests.push(request.clone())
        return { ...metric(1, 'startup'), rewritten: ['padded'] }
      },
    })
    const user = userEvent.setup()
    renderApp('/metrics')
    await table()

    await user.click(
      screen.getByRole('button', { name: 'Rename metric load_time' }),
    )
    const rename = within(await dialog('Rename load_time'))
    const confirm = rename.getByRole('button', { name: 'Rename' })
    // The current name and a taken one cannot be confirmed.
    expect(rename.getByText('This is the current name.')).toBeInTheDocument()
    expect(confirm).toBeDisabled()
    const name = rename.getByRole('textbox', { name: 'New name' })
    await user.clear(name)
    await enterText(user, name, 'fps')
    expect(rename.getByText('Another metric has this name.')).toBeVisible()
    expect(confirm).toBeDisabled()
    // What the rename changes and what it breaks.
    expect(rename.getByText('padded')).toBeInTheDocument()
    expect(
      rename.getByText(/harness has to upload under the new name/),
    ).toBeInTheDocument()

    await user.clear(name)
    await enterText(user, name, 'startup')
    await user.click(confirm)
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(await sent(requests, 'POST')).toEqual({ name: 'startup' })
  })

  it('deletes a metric without results only', async () => {
    const requests: Request[] = []
    setup({
      '/ui/api/v1/metrics/triangles': (_, request) => {
        requests.push(request.clone())
        return new Response(null, { status: 204 })
      },
    })
    const user = userEvent.setup()
    renderApp('/metrics')
    await table()

    await user.click(
      screen.getByRole('button', { name: 'Delete metric load_time' }),
    )
    const blocked = within(await dialog('Delete load_time?'))
    expect(blocked.getByText(/runs have results for it/)).toBeInTheDocument()
    expect(blocked.getByRole('button', { name: 'Delete' })).toBeDisabled()
    await user.click(blocked.getByRole('button', { name: 'Cancel' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )

    await user.click(
      screen.getByRole('button', { name: 'Delete metric triangles' }),
    )
    const remove = within(await dialog('Delete triangles?'))
    await user.click(remove.getByRole('button', { name: 'Delete' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(requests.map((r) => r.method)).toEqual(['DELETE'])
  })

  it('links a metric to the runs that measured it', async () => {
    setup()
    renderApp('/metrics')
    await table()
    expect(
      screen.getByRole('link', { name: 'Runs with metric load_time' }),
    ).toHaveAttribute('href', '/runs?metric=load_time')
  })

  it('searches through the URL', async () => {
    setup()
    const user = userEvent.setup()
    const router = renderApp('/metrics?q=frame')

    // The descriptions of load_time ("first frame") and triangles
    // ("per frame") match.
    expect(await bodyRows()).toHaveLength(2)

    const search = screen.getByRole('searchbox', { name: 'Search metrics' })
    await user.clear(search)
    await enterText(user, search, 'nothing')
    expect(currentQuery(router)).toBe('q=nothing')
    expect(
      await screen.findByText('No metrics match the search.'),
    ).toBeInTheDocument()
  })

  it('says when there are no metrics yet', async () => {
    setup({ '/ui/api/v1/metrics': () => [] })
    renderApp('/metrics')
    expect(await screen.findByText(/No metrics yet/)).toBeInTheDocument()
  })

  it('shows an error with retry', async () => {
    setup({
      '/ui/api/v1/metrics': () => problem(500, 'internal', 'database down'),
    })
    renderApp('/metrics')
    expect(
      await screen.findByText(/The metrics could not be loaded: database down/),
    ).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
  })

  it('shows the unit catalogue by quantity, with custom units first', async () => {
    setup()
    const user = userEvent.setup()
    const router = renderApp('/metrics')
    await table()

    await user.click(screen.getByRole('tab', { name: 'Unit catalogue' }))
    expect(currentQuery(router)).toBe('tab=units')

    const custom = await screen.findByRole('table', { name: 'Custom units' })
    expect(within(custom).getByText('tris')).toBeInTheDocument()
    expect(
      within(custom).getByRole('link', { name: 'Show metric triangles' }),
    ).toHaveAttribute('href', '/metrics?q=triangles')

    const time = screen.getByRole('table', { name: 'Time' })
    const rows = within(time).getAllByRole('row').slice(1)
    expect(
      rows.map((r) => within(r).getAllByRole('cell')[0]!.textContent),
    ).toEqual(['ms', 's', 'min'])
    expect(within(rows[0]!).getByText('0.001 s')).toBeInTheDocument()
    expect(within(rows[1]!).getByText('Base unit')).toBeInTheDocument()
    expect(within(rows[2]!).getByText('60 s')).toBeInTheDocument()
    expect(
      within(rows[1]!).getByRole('link', { name: 'Show metric load_time' }),
    ).toBeInTheDocument()
    expect(within(rows[0]!).queryByRole('link')).not.toBeInTheDocument()
    expect(
      screen.getByRole('table', { name: 'Frame rate' }),
    ).toBeInTheDocument()
    expect(screen.getByText('4 units')).toBeInTheDocument()
  })

  it('searches the unit catalogue', async () => {
    setup()
    renderApp('/metrics?tab=units&q=minute')
    const time = await screen.findByRole('table', { name: 'Time' })
    expect(within(time).getAllByRole('row')).toHaveLength(2)
    expect(
      screen.queryByRole('table', { name: 'Frame rate' }),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('table', { name: 'Custom units' }),
    ).not.toBeInTheDocument()
    expect(screen.getByText('1 unit')).toBeInTheDocument()
  })
})
