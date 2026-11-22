import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { Alias, TestCaseSummary } from '../api/client'
import { mockApi, problem } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

type Handler = NonNullable<Parameters<typeof mockApi>[0]>[string]

const alias = (
  id: number,
  dimension: string,
  source: unknown,
  target: unknown,
  version = 1,
): Alias => ({
  id,
  dimension,
  source: source as never,
  target: target as never,
  version,
  updated_at: '2026-11-20T10:00:00Z',
})

const aliases = [
  alias(1, 'client_device', 'pixel8-lab1', 'pixel-8-lab-1', 2),
  alias(2, 'parameter:size', 10, 20),
  alias(3, 'parameter:mode', 'old', null, 3),
]

const devices = [
  {
    id: 1,
    name: 'pixel8-lab1',
    resolved_name: 'pixel-8-lab-1',
    created_at: '2026-11-01T00:00:00Z',
  },
  {
    id: 2,
    name: 'pixel-8-lab-1',
    resolved_name: 'pixel-8-lab-1',
    created_at: '2026-11-01T00:00:00Z',
  },
  {
    id: 3,
    name: 'iphone-15',
    resolved_name: 'iphone-15',
    created_at: '2026-11-01T00:00:00Z',
  },
]

const testCase = (id: number, parameters: object): TestCaseSummary => ({
  id,
  scenario: 'load-model',
  scenario_version: 1,
  scenario_version_id: 1,
  parameters: parameters as never,
  resolved_parameters: parameters as never,
  parameter_units: {},
  assets: [],
  tags: [],
})

const testCases = [
  testCase(1, { size: 10, mode: 'fast' }),
  testCase(2, { size: '10', mode: 'old' }),
]

/** The API with the aliases above; PUT bodies are collected in `sent`. */
function setup(handlers: Record<string, Handler> = {}) {
  const sent: unknown[] = []
  const api = mockApi({
    '/ui/api/v1/aliases': async (_, request) => {
      if (request.method === 'PUT') {
        const body = (await request.json()) as {
          dimension: string
          source: unknown
          target: unknown
        }
        sent.push(body)
        return alias(99, body.dimension, body.source, body.target)
      }
      return aliases
    },
    '/ui/api/v1/client-devices': () => devices,
    '/ui/api/v1/test-cases': () => testCases,
    ...handlers,
  })
  return { api, sent }
}

const table = () => screen.findByRole('table', { name: 'Aliases' })
const bodyRows = async () =>
  within(await table())
    .getAllByRole('row')
    .slice(1)

describe('Aliases page', () => {
  it('lists the current aliases, removed ones on request', async () => {
    setup()
    const user = userEvent.setup()
    const router = renderApp('/aliases')

    const rows = await bodyRows()
    expect(rows).toHaveLength(2)
    const device = within(rows[0]!)
    expect(device.getByText('Client device')).toBeInTheDocument()
    expect(device.getByText('pixel8-lab1')).toBeInTheDocument()
    expect(device.getByText('pixel-8-lab-1')).toBeInTheDocument()
    expect(device.getByText('v2')).toBeInTheDocument()
    // Parameter values show their type.
    const size = within(rows[1]!)
    expect(size.getByText('size')).toBeInTheDocument()
    expect(size.getAllByText('Number')).toHaveLength(2)
    expect(screen.getByText('2 aliases')).toBeInTheDocument()

    await user.click(screen.getByRole('switch', { name: 'Show removed' }))
    expect(currentQuery(router)).toBe('removed=1')
    const all = await bodyRows()
    expect(all).toHaveLength(3)
    expect(within(all[2]!).getByText('Removed')).toBeInTheDocument()
    expect(
      within(all[2]!).getByRole('button', {
        name: 'Set the alias of old (Text) again',
      }),
    ).toBeInTheDocument()
  })

  it('filters by dimension and text through the URL', async () => {
    setup()
    const user = userEvent.setup()
    const router = renderApp('/aliases?dimension=parameter%3Asize')

    expect(await bodyRows()).toHaveLength(1)
    await user.click(screen.getByRole('combobox', { name: 'Dimension' }))
    await user.click(screen.getByRole('option', { name: 'Client devices' }))
    expect(currentQuery(router)).toBe('dimension=client_device')
    expect(await bodyRows()).toHaveLength(1)

    await user.click(screen.getByRole('combobox', { name: 'Dimension' }))
    await user.click(screen.getByRole('option', { name: 'All dimensions' }))
    await enterText(
      user,
      screen.getByRole('searchbox', { name: 'Search' }),
      'nothing',
    )
    expect(currentQuery(router)).toBe('q=nothing')
    expect(
      await screen.findByText('No aliases match the filters.'),
    ).toBeInTheDocument()
  })

  it('says when there are no aliases yet', async () => {
    setup({ '/ui/api/v1/aliases': () => [] })
    renderApp('/aliases')
    expect(await screen.findByText(/No aliases yet/)).toBeInTheDocument()
  })

  it('marks sources that are not in the results', async () => {
    setup({
      '/ui/api/v1/aliases': () => [alias(1, 'client_device', 'pixl8', 'x')],
    })
    renderApp('/aliases')
    const rows = await bodyRows()
    expect(
      await within(rows[0]!).findByText('Not in results'),
    ).toBeInTheDocument()
  })

  it('adds a device alias that merges two names', async () => {
    const { sent } = setup()
    const user = userEvent.setup()
    renderApp('/aliases')
    await table()

    await user.click(screen.getByRole('button', { name: 'Add alias' }))
    const dialog = await screen.findByRole('dialog', { name: 'Add alias' })
    const save = within(dialog).getByRole('button', { name: 'Save' })
    expect(save).toBeDisabled()

    await enterText(
      user,
      within(dialog).getByRole('combobox', { name: 'From' }),
      'iphone',
    )
    await user.click(await screen.findByRole('option', { name: 'iphone-15' }))
    // Keystrokes in the autocompletes are slow under load: pick, then append.
    const to = within(dialog).getByRole('combobox', { name: 'To' })
    await user.type(to, 'iph')
    await user.click(await screen.findByRole('option', { name: 'iphone-15' }))
    expect(within(dialog).getByText('From and To must differ')).toBeVisible()
    expect(save).toBeDisabled()

    await user.type(to, '-lab')
    expect(to).toHaveValue('iphone-15-lab')
    await user.click(save)

    await waitFor(() =>
      expect(sent).toEqual([
        {
          dimension: 'client_device',
          source: 'iphone-15',
          target: 'iphone-15-lab',
        },
      ]),
    )
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })

  it('keeps the JSON type of parameter values', async () => {
    const { sent } = setup()
    const user = userEvent.setup()
    renderApp('/aliases')
    await table()

    await user.click(screen.getByRole('button', { name: 'Add alias' }))
    const dialog = await screen.findByRole('dialog', { name: 'Add alias' })
    await user.click(within(dialog).getByRole('button', { name: 'Parameter' }))
    // The name is what to type next.
    expect(
      within(dialog).getByRole('combobox', { name: 'Parameter name' }),
    ).toHaveFocus()
    await user.keyboard('size')
    // The text "10" was uploaded too; pick it, then the number.
    await user.type(
      within(dialog).getByRole('combobox', { name: 'From' }),
      '10',
    )
    expect(
      await screen.findByRole('option', { name: '10 (Text)' }),
    ).toBeInTheDocument()
    await user.click(screen.getByRole('option', { name: '10 (Number)' }))
    // The target takes the source's type.
    await user.type(within(dialog).getByRole('combobox', { name: 'To' }), '12')
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))

    await waitFor(() =>
      expect(sent).toEqual([
        { dimension: 'parameter:size', source: 10, target: 12 },
      ]),
    )
  })

  it('tells what an alias replaces and that numbers must be numbers', async () => {
    setup()
    const user = userEvent.setup()
    renderApp('/aliases')
    await table()

    await user.click(screen.getByRole('button', { name: 'Add alias' }))
    const dialog = await screen.findByRole('dialog', { name: 'Add alias' })
    await user.click(within(dialog).getByRole('button', { name: 'Parameter' }))
    await user.type(
      within(dialog).getByRole('combobox', { name: 'Parameter name' }),
      'size',
    )
    await user.type(
      within(dialog).getByRole('combobox', { name: 'From' }),
      '10',
    )
    await user.click(
      within(await screen.findByRole('listbox')).getByRole('option', {
        name: '10 (Number)',
      }),
    )
    await user.type(within(dialog).getByRole('combobox', { name: 'To' }), 'big')
    expect(within(dialog).getByText('Not a number')).toBeVisible()
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeDisabled()

    await user.clear(within(dialog).getByRole('combobox', { name: 'To' }))
    await user.type(within(dialog).getByRole('combobox', { name: 'To' }), '30')
    expect(
      within(dialog).getByText(
        'Replaces the current alias to 20 (Number), as a new version.',
      ),
    ).toBeVisible()
  })

  it('changes the target of an alias', async () => {
    const { sent } = setup()
    const user = userEvent.setup()
    renderApp('/aliases')

    await user.click(
      await screen.findByRole('button', {
        name: 'Change the alias of pixel8-lab1',
      }),
    )
    const dialog = await screen.findByRole('dialog', { name: 'Change alias' })
    expect(
      within(dialog).getByRole('combobox', { name: 'From' }),
    ).toBeDisabled()
    const to = within(dialog).getByRole('combobox', { name: 'To' })
    expect(to).toHaveValue('pixel-8-lab-1')
    await user.clear(to)
    await enterText(user, to, 'pixel-8-a')
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))

    await waitFor(() =>
      expect(sent).toEqual([
        {
          dimension: 'client_device',
          source: 'pixel8-lab1',
          target: 'pixel-8-a',
        },
      ]),
    )
  })

  it('removes an alias after asking', async () => {
    const { api, sent } = setup()
    const user = userEvent.setup()
    renderApp('/aliases')

    await user.click(
      await screen.findByRole('button', {
        name: 'Remove the alias of 10 (Number)',
      }),
    )
    const dialog = await screen.findByRole('dialog', { name: 'Remove alias?' })
    expect(
      within(dialog).getByText(
        /Results show 10 \(Number\) again instead of 20/,
      ),
    ).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Remove' }))

    await waitFor(() =>
      expect(sent).toEqual([
        { dimension: 'parameter:size', source: 10, target: null },
      ]),
    )
    // Every view depends on aliases; the list is loaded again.
    await waitFor(() =>
      expect(api.requests('/ui/api/v1/aliases').length).toBeGreaterThanOrEqual(
        3,
      ),
    )
  })

  it('shows a rejected change in the dialog', async () => {
    setup({
      '/ui/api/v1/aliases': (_, request) =>
        request.method === 'PUT'
          ? problem(422, 'invalid', 'alias source and target must differ')
          : aliases,
    })
    const user = userEvent.setup()
    renderApp('/aliases')

    await user.click(
      await screen.findByRole('button', {
        name: 'Change the alias of pixel8-lab1',
      }),
    )
    const dialog = await screen.findByRole('dialog', { name: 'Change alias' })
    const to = within(dialog).getByRole('combobox', { name: 'To' })
    await user.clear(to)
    await user.type(to, 'other')
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))
    expect(
      await within(dialog).findByText('alias source and target must differ'),
    ).toBeInTheDocument()
  })

  it('shows chained aliases', async () => {
    setup({
      '/ui/api/v1/aliases': () => [
        alias(1, 'client_device', 'a', 'b'),
        alias(2, 'client_device', 'b', 'c'),
      ],
    })
    renderApp('/aliases')
    const rows = await bodyRows()
    expect(
      within(rows[0]!).getByText('Results show c (chained alias)'),
    ).toBeInTheDocument()
  })
})
