import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { CaseExecutionSummary, RunSummary } from '../../api/client'
import { formatDateTime } from '../../format'
import { mockApi, problem } from '../../test/api'
import { enterText } from '../../test/input'
import { currentQuery, renderApp } from '../../test/render'
import {
  caseExecution,
  casesPath,
  json,
  runDetail,
  runId,
  runPath,
} from './fixtures'

const aliased: CaseExecutionSummary = {
  ...caseExecution('ce1'),
  test_case: {
    ...caseExecution('ce1').test_case,
    parameters: json({ bandwidth: 20, pipeline: 'meshopt-v1' }),
    resolved_parameters: json({ bandwidth: 20, pipeline: 'meshopt' }),
  },
  client_device: { id: 2, name: 'pixel8-lab1', resolved_name: 'pixel-8-lab-1' },
}

const cases = [
  aliased,
  caseExecution('ce2', { scenario: 'zoom' }),
  caseExecution('ce3', { device: 'iphone-15-lab-2' }),
]

const noContent = () => new Response(null, { status: 204 })

const section = (name: string) => screen.findByRole('region', { name })
const caseRows = async () =>
  within(
    await screen.findByRole('table', { name: 'Case executions' }),
  )
    .getAllByRole('row')
    .slice(1)

describe('Run detail', () => {
  it('shows what the run is', async () => {
    mockApi({ [runPath]: () => runDetail(), [casesPath]: () => cases })
    renderApp(`/runs/${runId}`)

    const title = `streaming-nightly · ${formatDateTime('2026-11-20T03:05:00Z')}`
    expect(
      await screen.findByRole('heading', { level: 1, name: title }),
    ).toBeInTheDocument()
    expect(document.title).toBe(`${title} · Track Record`)

    const overview = within(await section('Overview'))
    expect(overview.getByText('Completed')).toBeInTheDocument()
    expect(overview.getByText('Nightly')).toBeInTheDocument()
    expect(overview.getByText('(39 min 00 s)')).toBeInTheDocument()
    expect(overview.getByText('168fed94')).toBeInTheDocument()
    expect(overview.getByText('main')).toBeInTheDocument()
    expect(overview.getByText('v1.5.0')).toBeInTheDocument()
    expect(overview.getByText('release')).toBeInTheDocument()
    expect(overview.getByText(runId)).toBeInTheDocument()
    // The suite opens the version the run was executed with.
    expect(
      overview.getByRole('link', { name: 'streaming-nightly' }),
    ).toHaveAttribute('href', '/suites?name=streaming-nightly&version=2')
    expect(screen.getByRole('link', { name: 'All runs' })).toHaveAttribute(
      'href',
      '/runs',
    )
  })

  it('shows tags, annotations, attachments and the shared environment', async () => {
    mockApi({ [runPath]: () => runDetail(), [casesPath]: () => cases })
    renderApp(`/runs/${runId}`)

    const tags = within(await section('Tags'))
    expect(
      tags.getByRole('button', { name: 'Show runs tagged nightly' }),
    ).toBeInTheDocument()
    expect(
      tags.getByRole('button', { name: 'Show runs tagged release' }),
    ).toBeInTheDocument()

    expect(
      within(await section('Annotations')).getByText(
        'Switched to the new compression',
      ),
    ).toBeInTheDocument()

    const attachments = within(await section('Attachments'))
    const log = attachments.getByRole('link', { name: 'Download harness.log' })
    expect(log).toHaveAttribute('href', '/ui/api/v1/attachments/f1/content')
    expect(log).toHaveAttribute('download', 'harness.log')
    expect(attachments.getByText('1.5 kB')).toBeInTheDocument()
    expect(attachments.getByText('2.4 MB')).toBeInTheDocument()
    expect(attachments.getByText('Run')).toBeInTheDocument()
    // Named after its case execution once those are loaded.
    expect(
      await attachments.findByRole('link', {
        name: 'Sample of zoom on pixel-8-lab-1',
      }),
    ).toHaveAttribute('href', `/runs/${runId}/cases/ce2`)

    const environment = within(await section('Shared environment'))
    expect(environment.getByText('backend.version')).toBeInTheDocument()
    expect(environment.getByText('2.4.0')).toBeInTheDocument()
    expect(environment.getByText('eu-central')).toBeInTheDocument()
    expect(
      environment.getByRole('link', { name: 'Runs with this environment' }),
    ).toHaveAttribute('href', '/runs?env=3')
  })

  it('lists the case executions with names and values after aliases', async () => {
    const user = userEvent.setup()
    mockApi({ [runPath]: () => runDetail(), [casesPath]: () => cases })
    renderApp(`/runs/${runId}`)

    const rows = await caseRows()
    expect(rows).toHaveLength(3)
    const first = within(rows[0]!)
    expect(first.getByRole('link', { name: 'orbit' })).toHaveAttribute(
      'href',
      `/runs/${runId}/cases/ce1`,
    )
    expect(first.getByText('bandwidth=20 Mbit/s')).toBeInTheDocument()
    expect(first.getByText('city-block 3')).toBeInTheDocument()
    expect(first.getByText('1 min 30 s')).toBeInTheDocument()

    await user.hover(first.getByText('pipeline=meshopt'))
    expect(await screen.findByRole('tooltip')).toHaveTextContent(
      'Uploaded as meshopt-v1',
    )
    await user.unhover(first.getByText('pipeline=meshopt'))
    await user.hover(first.getByText('pixel-8-lab-1'))
    await waitFor(() =>
      expect(screen.getByRole('tooltip')).toHaveTextContent(
        'Uploaded as pixel8-lab1',
      ),
    )
    expect(screen.getByText('3 case executions')).toBeInTheDocument()
  })

  it('opens a case execution from its row', async () => {
    const user = userEvent.setup()
    mockApi({ [runPath]: () => runDetail(), [casesPath]: () => cases })
    const router = renderApp(`/runs/${runId}`)

    const rows = await caseRows()
    await user.click(within(rows[1]!).getByText('4'))
    expect(router.state.location.pathname).toBe(`/runs/${runId}/cases/ce2`)

    // Not built yet; it leads back to its run.
    expect(
      screen.getByRole('heading', { level: 1, name: 'Case execution' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('link', { name: 'Back to the run' }),
    ).toHaveAttribute('href', `/runs/${runId}`)
  })

  it('filters the case executions through the URL', async () => {
    const user = userEvent.setup()
    mockApi({ [runPath]: () => runDetail(), [casesPath]: () => cases })
    const router = renderApp(`/runs/${runId}?device=iphone-15-lab-2`)

    expect(await caseRows()).toHaveLength(1)
    expect(screen.getByText('1 of 3 matches')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Clear filters' }))
    expect(currentQuery(router)).toBe('')
    expect(await caseRows()).toHaveLength(3)

    await user.type(screen.getByRole('searchbox', { name: 'Search' }), 'zoom')
    expect(currentQuery(router)).toBe('search=zoom')
    const rows = await caseRows()
    expect(rows).toHaveLength(1)
    expect(within(rows[0]!).getByText('zoom')).toBeInTheDocument()

    // Parameter values after aliases are searched as name=value.
    await user.clear(screen.getByRole('searchbox', { name: 'Search' }))
    await enterText(
      user,
      screen.getByRole('searchbox', { name: 'Search' }),
      'pipeline=meshopt',
    )
    expect(await caseRows()).toHaveLength(1)
  })

  it('shows superseded case executions on request', async () => {
    const user = userEvent.setup()
    const api = mockApi({
      [runPath]: () => runDetail(),
      [casesPath]: (url) =>
        url.searchParams.get('include_superseded') === 'true'
          ? [
              caseExecution('old', { superseded_by: 'new' }),
              caseExecution('new', { supersedes_id: 'old' }),
            ]
          : [caseExecution('new', { supersedes_id: 'old' })],
    })
    const router = renderApp(`/runs/${runId}`)

    expect(await caseRows()).toHaveLength(1)
    expect(screen.getByText('Correction')).toBeInTheDocument()

    await user.click(screen.getByRole('switch', { name: 'Show superseded' }))
    expect(currentQuery(router)).toBe('superseded=true')
    await waitFor(async () => expect(await caseRows()).toHaveLength(2))
    expect(screen.getByText('Superseded')).toBeInTheDocument()
    expect(
      api.requests(casesPath).at(-1)!.searchParams.get('include_superseded'),
    ).toBe('true')
  })

  it('adds and removes tags', async () => {
    const user = userEvent.setup()
    const sent: string[] = []
    let tags = ['nightly']
    const tagHandler = (url: URL, request: Request) => {
      const tag = decodeURIComponent(url.pathname.split('/').at(-1)!)
      sent.push(`${request.method} ${tag}`)
      tags =
        request.method === 'PUT'
          ? [...tags, tag]
          : tags.filter((t) => t !== tag)
      return noContent()
    }
    mockApi({
      [runPath]: () => runDetail({ tags }),
      [casesPath]: () => cases,
      [`${runPath}/tags/needs%20review`]: tagHandler,
      [`${runPath}/tags/nightly`]: tagHandler,
    })
    renderApp(`/runs/${runId}`)

    const tagSection = within(await section('Tags'))
    await enterText(
      user,
      tagSection.getByRole('textbox', { name: 'New tag' }),
      ' needs review ',
    )
    await user.keyboard('{Enter}')
    expect(
      await tagSection.findByRole('button', {
        name: 'Show runs tagged needs review',
      }),
    ).toBeInTheDocument()
    expect(tagSection.getByRole('textbox', { name: 'New tag' })).toHaveValue('')

    await user.click(tagSection.getByTitle('Remove tag nightly'))
    await waitFor(() =>
      expect(
        tagSection.queryByRole('button', { name: 'Show runs tagged nightly' }),
      ).not.toBeInTheDocument(),
    )

    // From the keyboard: Delete on a focused tag removes it.
    const added = tagSection.getByRole('button', {
      name: 'Show runs tagged needs review',
    })
    expect(added).toHaveAccessibleDescription('Press Delete to remove the tag.')
    added.focus()
    await user.keyboard('{Delete}')
    await waitFor(() =>
      expect(tagSection.getByText('No tags yet.')).toBeInTheDocument(),
    )
    expect(sent).toEqual([
      'PUT needs review',
      'DELETE nightly',
      'DELETE needs review',
    ])
  })

  it('opens the run list filtered by a tag', async () => {
    const user = userEvent.setup()
    mockApi({ [runPath]: () => runDetail(), [casesPath]: () => cases })
    const router = renderApp(`/runs/${runId}`)

    await user.click(
      await screen.findByRole('button', { name: 'Show runs tagged release' }),
    )
    expect(router.state.location.pathname).toBe('/runs')
    expect(currentQuery(router)).toBe('tag=release')
  })

  /** The run's annotations behind the API; returns the bodies sent. */
  function annotationsApi() {
    const bodies: unknown[] = []
    let annotations = runDetail().annotations
    mockApi({
      [runPath]: () => runDetail({ annotations }),
      [casesPath]: () => cases,
      '/ui/api/v1/annotations': async (_, request) => {
        const body = (await request.json()) as { body: string }
        bodies.push(body)
        const created = {
          id: 'a2',
          run_id: runId,
          body: body.body,
          created_at: '2026-11-21T10:00:00Z',
          updated_at: '2026-11-21T10:00:00Z',
        }
        annotations = [...annotations, created]
        return created
      },
      '/ui/api/v1/annotations/a1': async (_, request) => {
        if (request.method === 'DELETE') {
          annotations = annotations.filter((a) => a.id !== 'a1')
          return noContent()
        }
        const body = (await request.json()) as { body: string }
        bodies.push(body)
        annotations = annotations.map((a) =>
          a.id === 'a1'
            ? { ...a, body: body.body, updated_at: '2026-11-21T11:00:00Z' }
            : a,
        )
        return annotations[0]
      },
    })
    return bodies
  }

  /** The list item of the annotation the run starts with. */
  const firstAnnotation = async (notes: ReturnType<typeof within>) =>
    (await notes.findByText('Switched to the new compression')).closest('li')!

  it('adds an annotation', async () => {
    const user = userEvent.setup()
    const bodies = annotationsApi()
    renderApp(`/runs/${runId}`)
    const notes = within(await section('Annotations'))

    await enterText(
      user,
      notes.getByRole('textbox', { name: 'New annotation' }),
      'Lab moved to a new rack',
    )
    await user.click(notes.getByRole('button', { name: 'Add annotation' }))
    expect(await notes.findByText('Lab moved to a new rack')).toBeInTheDocument()
    expect(bodies).toEqual([{ body: 'Lab moved to a new rack', run_id: runId }])
  })

  it('edits an annotation', async () => {
    const user = userEvent.setup()
    const bodies = annotationsApi()
    renderApp(`/runs/${runId}`)
    const notes = within(await section('Annotations'))

    await user.click(
      within(await firstAnnotation(notes)).getByRole('button', {
        name: 'Edit annotation',
      }),
    )
    const editor = notes.getByRole('textbox', { name: 'Annotation' })
    expect(editor).toHaveFocus()
    await user.clear(editor)
    await enterText(user, editor, 'Switched to zstd')
    await user.click(notes.getByRole('button', { name: 'Save' }))
    const edited = (await notes.findByText('Switched to zstd')).closest('li')!
    expect(within(edited).getByText('· edited')).toBeInTheDocument()
    expect(bodies).toEqual([{ body: 'Switched to zstd' }])
  })

  it('deletes an annotation after asking', async () => {
    const user = userEvent.setup()
    annotationsApi()
    renderApp(`/runs/${runId}`)
    const notes = within(await section('Annotations'))

    await user.click(
      within(await firstAnnotation(notes)).getByRole('button', {
        name: 'Delete annotation',
      }),
    )
    const dialog = await screen.findByRole('dialog', {
      name: 'Delete this annotation?',
    })
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))
    await waitFor(() =>
      expect(
        notes.queryByText('Switched to the new compression'),
      ).not.toBeInTheDocument(),
    )
  })

  it('marks the run invalid with a reason and valid again', async () => {
    const user = userEvent.setup()
    const sent: unknown[] = []
    let validity = { is_valid: true, invalid_reason: null as string | null }
    mockApi({
      [runPath]: () => runDetail(validity),
      [casesPath]: () => cases,
      [`${runPath}/invalidate`]: async (_, request) => {
        const body = (await request.json()) as { reason: string }
        sent.push(body)
        validity = { is_valid: false, invalid_reason: body.reason }
        return noContent()
      },
      [`${runPath}/validate`]: () => {
        sent.push('validate')
        validity = { is_valid: true, invalid_reason: null }
        return noContent()
      },
    })
    renderApp(`/runs/${runId}`)

    await user.click(await screen.findByRole('button', { name: 'Mark invalid' }))
    const dialog = await screen.findByRole('dialog', {
      name: 'Mark this run invalid?',
    })
    const confirm = within(dialog).getByRole('button', { name: 'Mark invalid' })
    expect(confirm).toBeDisabled()
    const reason = within(dialog).getByRole('textbox', { name: 'Reason' })
    await waitFor(() => expect(reason).toHaveFocus())
    await enterText(user, reason, '  Throttled network  ')
    await user.click(confirm)

    expect(
      await screen.findByText('Marked invalid: Throttled network'),
    ).toBeInTheDocument()
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(
      screen.queryByRole('button', { name: 'Mark invalid' }),
    ).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Mark valid' }))
    await waitFor(() =>
      expect(
        screen.queryByText('Marked invalid: Throttled network'),
      ).not.toBeInTheDocument(),
    )
    expect(sent).toEqual([{ reason: 'Throttled network' }, 'validate'])
  })

  it('shows why an invalidation failed', async () => {
    const user = userEvent.setup()
    mockApi({
      [runPath]: () => runDetail(),
      [casesPath]: () => cases,
      [`${runPath}/invalidate`]: () =>
        problem(422, 'invalid', 'a reason is required to mark a run invalid'),
    })
    renderApp(`/runs/${runId}`)

    await user.click(await screen.findByRole('button', { name: 'Mark invalid' }))
    const dialog = await screen.findByRole('dialog')
    await user.type(within(dialog).getByRole('textbox'), 'x{Enter}')
    expect(
      await within(dialog).findByText(
        'a reason is required to mark a run invalid',
      ),
    ).toBeInTheDocument()
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('deletes the run only after its ID is typed', async () => {
    const user = userEvent.setup()
    const sent: unknown[] = []
    const remaining: RunSummary[] = []
    const api = mockApi({
      [runPath]: () => runDetail(),
      [casesPath]: () => cases,
      [`${runPath}/delete`]: async (_, request) => {
        sent.push(await request.json())
        return noContent()
      },
      '/ui/api/v1/runs': () => ({ runs: remaining, next_cursor: null }),
    })
    const router = renderApp(`/runs/${runId}`)

    await user.click(await screen.findByRole('button', { name: 'Delete' }))
    const dialog = await screen.findByRole('dialog', {
      name: 'Delete this run permanently?',
    })
    expect(within(dialog).getByText(runId)).toBeInTheDocument()
    const confirm = within(dialog).getByRole('button', {
      name: 'Delete permanently',
    })
    const input = within(dialog).getByRole('textbox', { name: 'Run ID' })
    expect(confirm).toBeDisabled()
    await enterText(user, input, runId.slice(0, -1))
    expect(confirm).toBeDisabled()
    await user.type(input, runId.slice(-1))
    expect(confirm).toBeEnabled()

    await user.click(confirm)
    await waitFor(() => expect(router.state.location.pathname).toBe('/runs'))
    expect(sent).toEqual([{ confirm_run_id: runId }])
    expect(await screen.findByText(/No runs yet/)).toBeInTheDocument()
    // The deleted run is dropped, not fetched again.
    expect(api.requests(runPath)).toHaveLength(1)
  })

  it('cancels a deletion without a request', async () => {
    const user = userEvent.setup()
    const api = mockApi({
      [runPath]: () => runDetail(),
      [casesPath]: () => cases,
    })
    renderApp(`/runs/${runId}`)

    await user.click(await screen.findByRole('button', { name: 'Delete' }))
    const dialog = await screen.findByRole('dialog')
    await enterText(user, within(dialog).getByRole('textbox'), runId)
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(api.requests(`${runPath}/delete`)).toHaveLength(0)

    // Opened again, it starts empty.
    await user.click(screen.getByRole('button', { name: 'Delete' }))
    expect(
      within(await screen.findByRole('dialog')).getByRole('textbox'),
    ).toHaveValue('')
  })

  it('says when there is no such run', async () => {
    mockApi({
      [runPath]: () => problem(404, 'not_found', `run ${runId} not found`),
    })
    renderApp(`/runs/${runId}`)
    expect(
      await screen.findByText(
        `There is no run ${runId}. It may have been deleted.`,
      ),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { level: 1, name: 'Run' }),
    ).toBeInTheDocument()
  })

  it('shows a failed request with a retry', async () => {
    const user = userEvent.setup()
    let fail = true
    mockApi({
      [runPath]: () =>
        fail ? problem(500, 'internal', 'database unavailable') : runDetail(),
      [casesPath]: () => cases,
    })
    renderApp(`/runs/${runId}`)

    expect(
      await screen.findByText(
        'The run could not be loaded: database unavailable',
      ),
    ).toBeInTheDocument()
    fail = false
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await section('Overview')).toBeInTheDocument()
  })
})
