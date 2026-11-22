import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { RunSummary } from '../api/client'
import { runPageSize } from './queries'
import { mockApi, problem } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

function run(overrides: Partial<RunSummary> & { id: string }): RunSummary {
  return {
    suite: { suite_version_id: 1, name: 'streaming-nightly', version: 1 },
    sut_version: {
      id: 1,
      commit: '168fed94a5013f65ea34157e7163f37114d6752d',
      branch: 'main',
      build_config: 'release',
      tag: null,
    },
    shared_environment_id: 1,
    triggered_by: 'nightly',
    status: 'completed',
    started_at: '2026-11-20T03:05:00Z',
    ended_at: '2026-11-20T03:44:00Z',
    is_valid: true,
    invalid_reason: null,
    created_at: '2026-11-21T22:18:29Z',
    tags: ['nightly'],
    ...overrides,
  }
}

const runs = [
  run({ id: 'r1', status: 'failed', tags: ['nightly', 'baseline'] }),
  run({
    id: 'r2',
    status: 'running',
    ended_at: null,
    triggered_by: 'ci',
    sut_version: {
      id: 2,
      commit: '2fe5025ffece20313c17ff32a24c8cd7af8aa14d',
      branch: 'fix/meshopt',
      build_config: 'debug',
      tag: 'v1.5.0',
    },
  }),
  run({ id: 'r3', is_valid: false, invalid_reason: 'Throttled network' }),
]

const table = () => screen.findByRole('table', { name: 'Runs' })
const bodyRows = async () =>
  within(await table()).getAllByRole('row').slice(1)

describe('Run list', () => {
  it('shows the runs of the API', async () => {
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    renderApp('/runs')

    const rows = await bodyRows()
    expect(rows).toHaveLength(3)
    const first = within(rows[0]!)
    expect(first.getByText('Failed')).toBeInTheDocument()
    expect(first.getByText('39 min 00 s')).toBeInTheDocument()
    expect(first.getByText('168fed94')).toBeInTheDocument()
    expect(first.getByText('baseline')).toBeInTheDocument()

    const second = within(rows[1]!)
    expect(second.getAllByText('Running')).toHaveLength(2) // status, duration
    expect(second.getByText('fix/meshopt')).toBeInTheDocument()
    expect(second.getByText('v1.5.0')).toBeInTheDocument()
    expect(second.getByText('CI')).toBeInTheDocument()

    expect(
      within(rows[2]!).getByLabelText('Invalid: Throttled network'),
    ).toBeInTheDocument()
    expect(screen.getByText('All 3 runs')).toBeInTheDocument()
  })

  it('links every run to its page', async () => {
    const user = userEvent.setup()
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    const router = renderApp('/runs')

    const rows = await bodyRows()
    const link = within(rows[1]!).getByRole('link', { name: /^Open run of/ })
    expect(link).toHaveAttribute('href', '/runs/r2')

    // The whole row opens the run, not just the link.
    await user.click(within(rows[0]!).getByText('streaming-nightly'))
    expect(router.state.location.pathname).toBe('/runs/r1')
  })

  it('does not open a run when text in its row is selected', async () => {
    const user = userEvent.setup()
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    const router = renderApp('/runs')
    const rows = await bodyRows()

    const selection = vi
      .spyOn(window, 'getSelection')
      .mockReturnValue({ toString: () => '168fed94' } as Selection)
    await user.click(within(rows[0]!).getByText('168fed94'))
    expect(router.state.location.pathname).toBe('/runs')
    selection.mockRestore()

    await user.keyboard('{Control>}')
    await user.click(within(rows[0]!).getByText('168fed94'))
    await user.keyboard('{/Control}')
    expect(router.state.location.pathname).toBe('/runs')
  })

  it('sends the filters of the URL to the API', async () => {
    const api = mockApi({
      '/ui/api/v1/runs': () => ({ runs: [], next_cursor: null }),
    })
    renderApp(
      '/runs?status=failed&status=aborted&trigger=ci&device=pixel-8&tag=nightly&validity=all',
    )
    await screen.findByText('No runs match these filters.')

    const sent = api.requests('/ui/api/v1/runs')[0]!.searchParams
    expect(sent.getAll('status')).toEqual(['failed', 'aborted'])
    expect(sent.get('triggered_by')).toBe('ci')
    expect(sent.get('client_device')).toBe('pixel-8')
    expect(sent.getAll('tag')).toEqual(['nightly'])
    expect(sent.get('validity')).toBe('all')
    expect(sent.get('limit')).toBe(String(runPageSize))
  })

  it('puts filter changes into the URL', async () => {
    const user = userEvent.setup()
    const api = mockApi({
      '/ui/api/v1/runs': () => ({ runs, next_cursor: null }),
    })
    const router = renderApp('/runs')
    await table()

    await user.click(screen.getByRole('combobox', { name: 'Trigger' }))
    await user.click(screen.getByRole('option', { name: 'Nightly' }))
    expect(currentQuery(router)).toBe('trigger=nightly')

    await user.click(screen.getByRole('button', { name: 'All' }))
    expect(currentQuery(router)).toBe('trigger=nightly&validity=all')

    await waitFor(() =>
      expect(
        api.requests('/ui/api/v1/runs').at(-1)!.searchParams.get('validity'),
      ).toBe('all'),
    )

    await user.click(screen.getByRole('button', { name: 'Clear filters' }))
    expect(currentQuery(router)).toBe('')
  })

  it('sends the more specific filters and shows them open', async () => {
    const api = mockApi({
      '/ui/api/v1/runs': () => ({ runs: [], next_cursor: null }),
    })
    renderApp(
      '/runs?commit=168fed94a5013f65ea34157e7163f37114d6752d&sutTag=v1.5.0' +
        '&build=release&env=3&scenario=load-model&asset=city&metric=fps' +
        '&param=size%3D10&param=mode%3Dfast',
    )
    await screen.findByText('No runs match these filters.')

    const sent = api.requests('/ui/api/v1/runs')[0]!.searchParams
    expect(sent.get('sut_commit')).toBe(
      '168fed94a5013f65ea34157e7163f37114d6752d',
    )
    expect(sent.get('sut_tag')).toBe('v1.5.0')
    expect(sent.get('sut_build_config')).toBe('release')
    expect(sent.get('shared_environment_id')).toBe('3')
    expect(sent.get('scenario')).toBe('load-model')
    expect(sent.get('asset')).toBe('city')
    expect(sent.get('metric')).toBe('fps')
    expect(sent.getAll('param')).toEqual(['size=10', 'mode=fast'])

    // A link with these filters opens "More filters", so they can be seen.
    expect(
      screen.getByRole('button', { name: 'Fewer filters' }),
    ).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('combobox', { name: 'Scenario' })).toHaveValue(
      'load-model',
    )
    expect(screen.getByText('size=10')).toBeInTheDocument()
    expect(screen.getByText('Shared environment 3')).toBeInTheDocument()
  })

  it('keeps the more specific filters closed until asked for', async () => {
    const user = userEvent.setup()
    const api = mockApi({
      '/ui/api/v1/runs': () => ({ runs, next_cursor: null }),
      '/ui/api/v1/scenarios': () => [
        { id: 1, name: 'load-model', versions: [] },
        { id: 2, name: 'stream-city', versions: [] },
      ],
    })
    const router = renderApp('/runs')
    await table()
    expect(
      screen.queryByRole('combobox', { name: 'Scenario' }),
    ).not.toBeInTheDocument()
    // Their options are not loaded while they are hidden.
    expect(api.requests('/ui/api/v1/scenarios')).toHaveLength(0)

    await user.click(screen.getByRole('button', { name: 'More filters' }))
    await user.click(screen.getByRole('combobox', { name: 'Scenario' }))
    await user.click(await screen.findByRole('option', { name: 'stream-city' }))
    expect(currentQuery(router)).toBe('scenario=stream-city')

    // Closed again, the button says how many of them are set.
    await user.click(screen.getByRole('button', { name: 'Fewer filters' }))
    expect(
      screen.getByRole('button', { name: 'More filters (1 set)' }),
    ).toHaveAttribute('aria-expanded', 'false')
  })

  it('takes parameter filters as name=value', async () => {
    const user = userEvent.setup()
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    const router = renderApp('/runs')
    await table()
    await user.click(screen.getByRole('button', { name: 'More filters' }))
    const input = screen.getByRole('combobox', { name: 'Parameters' })

    await enterText(user, input, ' size = 10 ')
    await user.keyboard('{Enter}')
    expect(currentQuery(router)).toBe('param=size%3D10')

    // Not name=value: the text stays to be fixed, nothing is filtered.
    await user.type(input, 'fast{Enter}')
    expect(currentQuery(router)).toBe('param=size%3D10')
    expect(input).toHaveValue('fast')
    expect(screen.getByText(/^Use name=value/)).toBeInTheDocument()

    await user.clear(input)
    await enterText(user, input, 'mode=fast')
    await user.keyboard('{Enter}')
    expect(currentQuery(router)).toBe('param=size%3D10&param=mode%3Dfast')
    expect(screen.queryByText(/^Use name=value/)).not.toBeInTheDocument()
  })

  it('removes the shared environment filter', async () => {
    const user = userEvent.setup()
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    const router = renderApp('/runs?env=3&validity=all')
    await table()

    const chip = screen.getByRole('button', { name: 'Shared environment 3' })
    await user.click(within(chip).getByTestId('CancelIcon'))
    expect(currentQuery(router)).toBe('validity=all')
  })

  it('keeps the rows on screen while another filter loads', async () => {
    const user = userEvent.setup()
    mockApi({
      '/ui/api/v1/runs': (url) =>
        url.searchParams.get('validity') === 'all'
          ? new Promise(() => {}) // never answers
          : { runs, next_cursor: null },
    })
    const router = renderApp('/runs')
    expect(await bodyRows()).toHaveLength(3)

    await user.click(screen.getByRole('button', { name: 'All' }))
    expect(currentQuery(router)).toBe('validity=all')
    expect(await bodyRows()).toHaveLength(3)
  })

  it('puts only complete dates into the URL', async () => {
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    const router = renderApp('/runs')
    await table()
    const from = screen.getByLabelText('Started from')

    // A year being typed: 0002, 0020, 0202, then 2026.
    for (const year of ['0002', '0020', '0202']) {
      fireEvent.change(from, { target: { value: `${year}-11-01` } })
      expect(currentQuery(router)).toBe('')
    }
    fireEvent.change(from, { target: { value: '2026-11-01' } })
    expect(currentQuery(router)).toBe('from=2026-11-01')

    fireEvent.change(from, { target: { value: '' } })
    expect(currentQuery(router)).toBe('')
  })

  it('shows a device of the URL that is not among the options', async () => {
    renderApp('/runs?device=pixel8-lab1')
    expect(
      await screen.findByRole('combobox', { name: 'Client device' }),
    ).toHaveValue('pixel8-lab1')
  })

  it('filters by a tag when its chip is clicked', async () => {
    const user = userEvent.setup()
    mockApi({ '/ui/api/v1/runs': () => ({ runs, next_cursor: null }) })
    const router = renderApp('/runs')
    const rows = await bodyRows()

    await user.click(
      within(rows[0]!).getByRole('button', { name: 'Filter by tag baseline' }),
    )
    expect(router.state.location.pathname).toBe('/runs')
    expect(currentQuery(router)).toBe('tag=baseline')
  })

  it('loads more runs page by page', async () => {
    const user = userEvent.setup()
    const api = mockApi({
      '/ui/api/v1/runs': (url) =>
        url.searchParams.get('cursor') === 'page-2'
          ? { runs: [runs[2]], next_cursor: null }
          : { runs: runs.slice(0, 2), next_cursor: 'page-2' },
    })
    renderApp('/runs')

    expect(await bodyRows()).toHaveLength(2)
    expect(screen.getByText('2 runs shown')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Load more' }))
    await waitFor(async () => expect(await bodyRows()).toHaveLength(3))
    expect(screen.getByText('All 3 runs')).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Load more' }),
    ).not.toBeInTheDocument()
    expect(
      api.requests('/ui/api/v1/runs').map((u) => u.searchParams.get('cursor')),
    ).toEqual([null, 'page-2'])
  })

  it('keeps the loaded runs when loading more fails', async () => {
    const user = userEvent.setup()
    let fail = true
    const api = mockApi({
      '/ui/api/v1/runs': (url) =>
        url.searchParams.get('cursor') !== 'page-2'
          ? { runs: runs.slice(0, 2), next_cursor: 'page-2' }
          : fail
            ? problem(502, 'bad_gateway', 'upstream down')
            : { runs: [runs[2]], next_cursor: null },
    })
    renderApp('/runs')
    expect(await bodyRows()).toHaveLength(2)

    await user.click(screen.getByRole('button', { name: 'Load more' }))
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('More runs could not be loaded: upstream down')
    expect(await bodyRows()).toHaveLength(2)

    fail = false
    await user.click(within(alert).getByRole('button', { name: 'Retry' }))
    await waitFor(async () => expect(await bodyRows()).toHaveLength(3))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    // The retry asks for the next page only, not the first one again.
    expect(
      api.requests('/ui/api/v1/runs').map((u) => u.searchParams.get('cursor')),
    ).toEqual([null, 'page-2', 'page-2'])
  })

  it('says when there are no runs at all', async () => {
    renderApp('/runs')
    expect(
      await screen.findByText(
        'No runs yet. Runs appear here once the benchmark harness uploads them.',
      ),
    ).toBeInTheDocument()
  })

  it('shows API errors and retries', async () => {
    const user = userEvent.setup()
    let fail = true
    mockApi({
      '/ui/api/v1/runs': () =>
        fail
          ? problem(400, 'invalid_filter', 'unknown suite')
          : { runs, next_cursor: null },
    })
    renderApp('/runs')

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('The runs could not be loaded: unknown suite')

    fail = false
    await user.click(within(alert).getByRole('button', { name: 'Retry' }))
    expect(await bodyRows()).toHaveLength(3)
  })
})
