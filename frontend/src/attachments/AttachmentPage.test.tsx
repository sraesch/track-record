import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { AttachmentMeta } from '../api/client'
import { runId } from '../runs/detail/fixtures'
import { mockApi, problem } from '../test/api'
import { renderApp } from '../test/render'

function attachment(
  id: string,
  file_name: string,
  content_type: string,
  run_id = runId,
): AttachmentMeta {
  return {
    id,
    run_id,
    file_name,
    content_type,
    size_bytes: 127,
    sha256: 'ab',
    created_at: '2026-11-20T03:44:00Z',
  }
}

const metaPath = (id: string) => `/ui/api/v1/attachments/${id}`
const contentPath = (id: string) => `/ui/api/v1/attachments/${id}/content`

describe('Attachment page', () => {
  it('shows an attachment on its own page', async () => {
    mockApi({
      [metaPath('md')]: () =>
        attachment('md', 'summary.md', 'text/markdown; charset=utf-8'),
      [contentPath('md')]: () => new Response('# Nightly summary\n\nAll passed.'),
    })
    renderApp(`/runs/${runId}/attachments/md`)

    expect(
      await screen.findByRole('heading', { level: 1, name: 'summary.md' }),
    ).toBeInTheDocument()
    expect(document.title).toBe('summary.md · Track Record')
    expect(screen.getByText('text/markdown · 127 bytes')).toBeInTheDocument()
    expect(
      await screen.findByRole('heading', { name: 'Nightly summary' }),
    ).toBeInTheDocument()
    expect(screen.getByText('All passed.')).toBeInTheDocument()

    expect(
      screen.getByRole('link', { name: 'Back to the run' }),
    ).toHaveAttribute('href', `/runs/${runId}`)
    expect(screen.getByRole('link', { name: 'Download' })).toHaveAttribute(
      'href',
      contentPath('md'),
    )
  })

  it('shows an image at its own size', async () => {
    mockApi({
      [metaPath('img')]: () => attachment('img', 'screenshot.png', 'image/png'),
    })
    renderApp(`/runs/${runId}/attachments/img`)

    const image = await screen.findByRole('img', { name: 'screenshot.png' })
    expect(image).toHaveAttribute('src', contentPath('img'))
    expect(getComputedStyle(image).maxHeight).not.toMatch(/calc/)
  })

  it('offers files it cannot show for download', async () => {
    mockApi({
      [metaPath('bin')]: () =>
        attachment('bin', 'trace.bin', 'application/octet-stream'),
    })
    renderApp(`/runs/${runId}/attachments/bin`)

    expect(
      await screen.findByText(
        'This file cannot be shown here. Download it to open it.',
      ),
    ).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Download' })).toBeInTheDocument()
  })

  it('corrects a link with the wrong run', async () => {
    mockApi({
      [metaPath('log')]: () => attachment('log', 'harness.log', 'text/plain'),
      [contentPath('log')]: () => new Response('INFO start'),
    })
    const router = renderApp('/runs/other-run/attachments/log')

    expect(await screen.findByText('INFO start')).toBeInTheDocument()
    expect(router.state.location.pathname).toBe(
      `/runs/${runId}/attachments/log`,
    )
  })

  it('says when there is no such attachment', async () => {
    mockApi({
      [metaPath('gone')]: () =>
        problem(404, 'not_found', 'attachment gone not found'),
    })
    renderApp(`/runs/${runId}/attachments/gone`)

    expect(
      await screen.findByText(
        'There is no attachment gone. It may have been deleted with its run.',
      ),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { level: 1, name: 'Attachment' }),
    ).toBeInTheDocument()
  })
})
