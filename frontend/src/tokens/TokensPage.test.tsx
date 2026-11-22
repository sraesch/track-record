import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { ApiToken } from '../api/client'
import { mockApi, problem } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

const token = (
  id: string,
  name: string,
  fields: Partial<ApiToken> = {},
): ApiToken => ({
  id,
  name,
  scope: 'read',
  created_at: '2026-11-01T08:00:00Z',
  last_used_at: null,
  revoked_at: null,
  ...fields,
})

// Newest first, as the server lists them.
const tokens: ApiToken[] = [
  token('t3', 'assistant', { scope: 'annotate' }),
  token('t2', 'harness', {
    scope: 'write',
    last_used_at: '2026-11-20T09:11:00Z',
  }),
  token('t1', 'old harness', {
    scope: 'write',
    revoked_at: '2026-11-02T08:00:00Z',
  }),
]

const secret = 'trk_c2VjcmV0LXNlY3JldC1zZWNyZXQtc2VjcmV0LXNlY3JldA'

const list = '/ui/api/v1/tokens'
const table = () => screen.findByRole('table', { name: 'API tokens' })
const bodyRows = async () =>
  within(await table()).getAllByRole('row').slice(1)
const names = (rows: HTMLElement[]) =>
  rows.map((r) => within(r).getAllByRole('cell')[0]!.textContent)
const dialog = (name: string) => screen.findByRole('dialog', { name })
const closed = () =>
  waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

describe('API tokens page', () => {
  it('lists the active tokens with scope and last use', async () => {
    mockApi({ [list]: () => tokens })
    renderApp('/tokens')

    const rows = await bodyRows()
    expect(names(rows)).toEqual(['assistant', 'harness'])
    const assistant = within(rows[0]!)
    expect(assistant.getByText('Annotate')).toBeInTheDocument()
    expect(assistant.getByText('Never')).toBeInTheDocument()
    expect(assistant.getByText('Active')).toBeInTheDocument()
    const harness = within(rows[1]!)
    expect(harness.getByText('Write')).toBeInTheDocument()
    expect(harness.queryByText('Never')).not.toBeInTheDocument()
    expect(
      screen.getByText('2 tokens · 1 revoked token is hidden'),
    ).toBeInTheDocument()
  })

  it('lists revoked tokens on request, through the URL', async () => {
    mockApi({ [list]: () => tokens })
    const user = userEvent.setup()
    const router = renderApp('/tokens')
    await table()

    await user.click(screen.getByRole('switch', { name: 'Show revoked' }))
    expect(currentQuery(router)).toBe('revoked=1')
    const rows = await bodyRows()
    expect(names(rows)).toEqual(['assistant', 'harness', 'old harness'])
    const old = within(rows[2]!)
    expect(old.getByText('Revoked')).toBeInTheDocument()
    // A revoked token cannot be revoked again.
    expect(old.queryByRole('button')).not.toBeInTheDocument()
    expect(screen.getByText('3 tokens')).toBeInTheDocument()
  })

  it('creates a token and shows its secret once', async () => {
    const requests: Request[] = []
    const stored = [...tokens]
    mockApi({
      [list]: (_, request) => {
        if (request.method !== 'POST') return [...stored]
        requests.push(request.clone())
        const created = token('t4', 'ci harness', { scope: 'write' })
        stored.unshift(created)
        return new Response(JSON.stringify({ ...created, secret }), {
          status: 201,
          headers: { 'content-type': 'application/json' },
        })
      },
    })
    const user = userEvent.setup()
    renderApp('/tokens')
    await table()

    await user.click(screen.getByRole('button', { name: 'Create token' }))
    const create = within(await dialog('Create API token'))
    const confirm = create.getByRole('button', { name: 'Create' })
    // A token needs a name; the scope starts at the least one.
    expect(confirm).toBeDisabled()
    expect(create.getByRole('radio', { name: /^Read/ })).toBeChecked()
    const name = create.getByRole('textbox', { name: 'Name' })
    await enterText(user, name, 'harness')
    expect(
      create.getByText(/An active token already has this name/),
    ).toBeInTheDocument()
    await user.clear(name)
    await enterText(user, name, ' ci harness ')
    await user.click(create.getByRole('radio', { name: /^Write/ }))
    await user.click(confirm)

    const created = within(await dialog('Token ci harness created'))
    expect(await requests[0]!.json()).toEqual({
      name: 'ci harness',
      scope: 'write',
    })
    expect(created.getByText(/cannot be shown again/)).toBeInTheDocument()
    expect(created.getByRole('textbox', { name: 'Token' })).toHaveValue(secret)
    await user.click(created.getByRole('button', { name: 'Copy token' }))
    expect(await navigator.clipboard.readText()).toBe(secret)

    // Escape does not close it: the secret would be lost by accident.
    await user.keyboard('{Escape}')
    expect(created.getByRole('textbox', { name: 'Token' })).toBeVisible()
    await user.click(created.getByRole('button', { name: 'Done' }))
    await closed()
    expect(screen.queryByDisplayValue(secret)).not.toBeInTheDocument()
    expect(names(await bodyRows())).toEqual([
      'ci harness',
      'assistant',
      'harness',
    ])
  })

  it('shows why a token could not be created', async () => {
    mockApi({
      [list]: (_, request) =>
        request.method === 'POST'
          ? problem(500, 'internal', 'database down')
          : tokens,
    })
    const user = userEvent.setup()
    renderApp('/tokens')
    await table()

    await user.click(screen.getByRole('button', { name: 'Create token' }))
    const create = within(await dialog('Create API token'))
    await enterText(user, create.getByRole('textbox', { name: 'Name' }), 'x')
    await user.click(create.getByRole('button', { name: 'Create' }))
    expect(await create.findByText('database down')).toBeInTheDocument()
  })

  it('revokes a token after asking', async () => {
    const requests: Request[] = []
    const stored = [...tokens]
    mockApi({
      [list]: () => [...stored],
      '/ui/api/v1/tokens/t2/revoke': (_, request) => {
        requests.push(request.clone())
        stored[1] = { ...stored[1]!, revoked_at: '2026-11-22T10:00:00Z' }
        return stored[1]
      },
    })
    const user = userEvent.setup()
    renderApp('/tokens')
    await table()

    await user.click(
      screen.getByRole('button', { name: 'Revoke token harness' }),
    )
    const revoke = within(await dialog('Revoke harness?'))
    expect(revoke.getByText(/rejected from now on/)).toBeInTheDocument()
    expect(revoke.getByText(/It was last used on/)).toBeInTheDocument()
    await user.click(revoke.getByRole('button', { name: 'Revoke' }))
    await closed()

    expect(requests.map((r) => r.method)).toEqual(['POST'])
    await waitFor(async () =>
      expect(names(await bodyRows())).toEqual(['assistant']),
    )
    expect(
      screen.getByText('1 token · 2 revoked tokens are hidden'),
    ).toBeInTheDocument()
  })

  it('says when there are no tokens yet', async () => {
    mockApi({ [list]: () => [] })
    renderApp('/tokens')
    expect(await screen.findByText(/No API tokens yet/)).toBeInTheDocument()
  })

  it('says when all tokens are revoked', async () => {
    mockApi({ [list]: () => [tokens[2]!] })
    renderApp('/tokens')
    expect(
      await screen.findByText('No active tokens. 1 revoked token is hidden.'),
    ).toBeInTheDocument()
  })

  it('shows an error with retry', async () => {
    mockApi({ [list]: () => problem(500, 'internal', 'database down') })
    renderApp('/tokens')
    expect(
      await screen.findByText(
        /The API tokens could not be loaded: database down/,
      ),
    ).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
  })
})
