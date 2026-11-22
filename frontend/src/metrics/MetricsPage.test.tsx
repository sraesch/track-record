import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { Metric, Unit } from '../api/client'
import { mockApi, problem } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

const metrics: Metric[] = [
  {
    id: 1,
    name: 'load_time',
    versions: [
      {
        id: 10,
        version: 1,
        unit: 'ms',
        value_type: 'float',
        direction: 'lower_is_better',
        description: 'Until the first frame.',
        created_at: '2026-10-01T08:00:00Z',
      },
      {
        id: 11,
        version: 2,
        unit: 's',
        value_type: 'float',
        direction: 'lower_is_better',
        description: 'Until the first frame.',
        created_at: '2026-11-01T08:00:00Z',
      },
    ],
  },
  {
    id: 2,
    name: 'fps',
    versions: [
      {
        id: 20,
        version: 1,
        unit: 'fps',
        value_type: 'float',
        direction: 'higher_is_better',
        description: '',
        created_at: '2026-10-01T08:00:00Z',
      },
    ],
  },
  {
    id: 3,
    name: 'triangles',
    versions: [
      {
        id: 30,
        version: 1,
        unit: 'tris',
        value_type: 'integer',
        direction: 'lower_is_better',
        description: 'Triangles drawn per frame.',
        created_at: '2026-10-01T08:00:00Z',
      },
    ],
  },
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
  within(await table())
    .getAllByRole('row')
    .slice(1)
    // Rows of the version tables are nested in their own table.
    .filter(
      (row) =>
        row.closest('table')?.getAttribute('aria-label') ===
        'Metric definitions',
    )
    .filter((row) => within(row).queryAllByRole('cell').length > 1)

describe('Metrics page', () => {
  it('lists the metrics with their latest definition', async () => {
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

  it('shows older versions with what they changed', async () => {
    setup()
    const user = userEvent.setup()
    renderApp('/metrics')
    await table()

    const toggle = screen.getByRole('button', { name: /v2 of 2/ })
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await user.click(toggle)
    expect(toggle).toHaveAttribute('aria-expanded', 'true')

    const versions = await screen.findByRole('table', {
      name: 'Versions of load_time',
    })
    const rows = within(versions).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(2)
    // v2 changed the unit from ms to s; v1 is the first version.
    expect(within(rows[0]!).getByText('v2')).toBeInTheDocument()
    expect(within(rows[0]!).getAllByText('Changed')).toHaveLength(1)
    expect(within(rows[1]!).getByText('ms')).toBeInTheDocument()
    expect(within(rows[1]!).queryByText('Changed')).not.toBeInTheDocument()
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
    // load_time uses s in its latest version, not ms any more.
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
