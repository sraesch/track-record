import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { mockApi } from '../test/api'
import { renderApp } from '../test/render'
import {
  caseExecution,
  casesPath,
  runDetail,
  runId,
  runPath,
} from '../runs/detail/fixtures'

// The Markdown view is loaded on first use. Here its code cannot be
// fetched, as after a release replaced it or a dev server lost its cache.
vi.mock('./MarkdownView', () => {
  throw new TypeError('Failed to fetch dynamically imported module')
})

describe('Attachment preview', () => {
  it('shows a view that fails to load in the dialog, not as a broken page', async () => {
    const user = userEvent.setup()
    mockApi({
      [runPath]: () =>
        runDetail({
          attachments: [
            {
              id: 'md',
              run_id: runId,
              file_name: 'summary.md',
              content_type: 'text/markdown',
              size_bytes: 20,
              sha256: 'ab',
              created_at: '2026-11-20T03:44:00Z',
            },
          ],
        }),
      [casesPath]: () => [caseExecution('ce1')],
      '/ui/api/v1/attachments/md/content': () => new Response('# Summary'),
    })
    renderApp(`/runs/${runId}`)

    await user.click(
      await screen.findByRole('link', { name: 'Open summary.md' }),
    )
    const dialog = within(
      await screen.findByRole('dialog', { name: 'summary.md' }),
    )
    expect(
      await dialog.findByText(/^The preview could not be shown: /),
    ).toBeInTheDocument()
    expect(dialog.getByRole('button', { name: 'Reload' })).toBeInTheDocument()
    expect(dialog.getByRole('link', { name: 'Download' })).toBeInTheDocument()

    // The run page is still there behind the dialog.
    expect(
      screen.queryByText('Something went wrong'),
    ).not.toBeInTheDocument()
  })
})
