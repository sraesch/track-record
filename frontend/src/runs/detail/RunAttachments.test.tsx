import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { AttachmentMeta } from '../../api/client'
import { mockApi, problem } from '../../test/api'
import { currentQuery, renderApp } from '../../test/render'
import { caseExecution, casesPath, runDetail, runId, runPath } from './fixtures'

function attachment(
  id: string,
  file_name: string,
  content_type: string,
  size_bytes = 1000,
): AttachmentMeta {
  return {
    id,
    run_id: runId,
    file_name,
    content_type,
    size_bytes,
    sha256: 'ab',
    created_at: '2026-11-20T03:44:00Z',
  }
}

const attachments = [
  attachment('img', 'screenshot.png', 'image/png', 5780),
  attachment('md', 'summary.md', 'text/markdown; charset=utf-8'),
  attachment('json', 'environment.json', 'application/json'),
  attachment('log', 'harness.log', 'text/plain'),
  attachment('bin', 'trace.bin', 'application/octet-stream'),
  attachment('html', 'report.html', 'text/html'),
  attachment('big', 'samples.json', 'application/json', 2_000_000),
  attachment('huge', 'capture.png', 'image/png', 8_000_000),
]

const contentPath = (id: string) => `/ui/api/v1/attachments/${id}/content`
const text = (body: string, type = 'text/plain') => () =>
  new Response(body, { headers: { 'content-type': type } })

const markdown = `# Nightly summary

All **24** case executions passed.

| device | result |
|---|---|
| pixel | ok |

<script>alert(1)</script>

Some <b>raw</b> HTML.

[docs](https://example.com/docs) and ![chart](https://example.com/chart.png)
`

function setup(handlers: Record<string, () => unknown> = {}) {
  return mockApi({
    [runPath]: () => runDetail({ attachments }),
    [casesPath]: () => [caseExecution('ce1')],
    [contentPath('md')]: text(markdown, 'text/markdown'),
    [contentPath('json')]: text(
      '{"harness":"bench 0.9","runs":3,"ok":true,"note":null}',
      'application/json',
    ),
    [contentPath('log')]: text('INFO start\nERROR lost device\n'),
    ...handlers,
  })
}

const table = () => screen.findByRole('table', { name: 'Attachments' })
const openLink = async (file: string) =>
  within(await table()).getByRole('link', { name: `Open ${file}` })

async function open(file: string) {
  const user = userEvent.setup()
  await user.click(await openLink(file))
  return screen.findByRole('dialog', { name: file })
}

describe('Run attachments', () => {
  it('lists files with thumbnails, preview links and downloads', async () => {
    setup()
    renderApp(`/runs/${runId}`)
    const list = within(await table())

    const screenshot = await openLink('screenshot.png')
    expect(screenshot).toHaveAttribute('href', `/runs/${runId}?attachment=img`)
    expect(screenshot.querySelector('img')).toHaveAttribute(
      'src',
      contentPath('img'),
    )
    // Too large for a thumbnail: an icon instead.
    expect((await openLink('capture.png')).querySelector('img')).toBeNull()
    expect(await openLink('summary.md')).not.toBeNull()
    expect(await openLink('harness.log')).not.toBeNull()

    // Only downloads: unknown types and HTML (never rendered).
    for (const file of ['trace.bin', 'report.html']) {
      expect(
        list.queryByRole('link', { name: `Open ${file}` }),
      ).not.toBeInTheDocument()
      expect(list.getByText(file)).toBeInTheDocument()
    }

    for (const a of attachments) {
      const download = list.getByRole('link', {
        name: `Download ${a.file_name}`,
      })
      expect(download).toHaveAttribute('href', contentPath(a.id))
      expect(download).toHaveAttribute('download', a.file_name)
    }
  })

  it('shows an image, with a download, and closes back to the run', async () => {
    const user = userEvent.setup()
    setup()
    const router = renderApp(`/runs/${runId}`)

    const dialog = await open('screenshot.png')
    expect(currentQuery(router)).toBe('attachment=img')
    expect(within(dialog).getByText('image/png · 5.8 kB')).toBeInTheDocument()
    expect(
      within(dialog).getByRole('img', { name: 'screenshot.png' }),
    ).toHaveAttribute('src', contentPath('img'))
    expect(
      within(dialog).getByRole('link', { name: 'Download' }),
    ).toHaveAttribute('href', contentPath('img'))

    await user.click(within(dialog).getByRole('button', { name: 'Close' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(router.state.location.pathname).toBe(`/runs/${runId}`)
    expect(currentQuery(router)).toBe('')
  })

  it('renders Markdown without its HTML and without loading its images', async () => {
    setup()
    renderApp(`/runs/${runId}`)
    const dialog = within(await open('summary.md'))

    expect(
      await dialog.findByRole('heading', { name: 'Nightly summary' }),
    ).toBeInTheDocument()
    expect(dialog.getByText('24').tagName).toBe('STRONG')
    expect(dialog.getByRole('cell', { name: 'pixel' })).toBeInTheDocument()

    const content = screen.getByRole('dialog')
    expect(content.querySelector('script')).toBeNull()
    expect(content.querySelector('b')).toBeNull()
    expect(content.querySelector('img')).toBeNull()

    const docs = dialog.getByRole('link', { name: 'docs' })
    expect(docs).toHaveAttribute('href', 'https://example.com/docs')
    expect(docs).toHaveAttribute('target', '_blank')
    expect(docs).toHaveAttribute('rel', 'noopener noreferrer')
    expect(dialog.getByRole('link', { name: 'Image: chart' })).toHaveAttribute(
      'href',
      'https://example.com/chart.png',
    )
  })

  it('pretty-prints and highlights JSON', async () => {
    setup()
    renderApp(`/runs/${runId}`)
    const dialog = within(await open('environment.json'))

    expect(await dialog.findByText('"harness"')).toHaveAttribute(
      'data-token',
      'key',
    )
    expect(dialog.getByText('"bench 0.9"')).toHaveAttribute(
      'data-token',
      'string',
    )
    expect(dialog.getByText('3')).toHaveAttribute('data-token', 'number')
    expect(dialog.getByText('true')).toHaveAttribute('data-token', 'boolean')
    expect(dialog.getByText('null')).toHaveAttribute('data-token', 'null')
    expect(dialog.getByText('"harness"').closest('pre')).toHaveTextContent(
      /^\{ "harness": "bench 0\.9", "runs": 3, "ok": true, "note": null \}$/,
    )
  })

  it('shows invalid JSON as uploaded, with a note', async () => {
    setup({ [contentPath('json')]: text('{"runs": NaN}') })
    renderApp(`/runs/${runId}`)
    const dialog = within(await open('environment.json'))

    expect(
      await dialog.findByText(
        'This is not valid JSON; it is shown as uploaded.',
      ),
    ).toBeInTheDocument()
    expect(dialog.getByText('NaN')).toHaveAttribute('data-token', 'other')
  })

  it('shows text as uploaded', async () => {
    setup()
    renderApp(`/runs/${runId}`)
    const dialog = within(await open('harness.log'))
    expect(
      await dialog.findByText(/ERROR lost device/),
    ).toHaveTextContent('INFO start ERROR lost device')
  })

  it('does not load text files too large to show', async () => {
    const api = setup()
    renderApp(`/runs/${runId}`)
    const dialog = within(await open('samples.json'))

    expect(
      dialog.getByText(/This file is too large to show here \(2 MB; up to 1 MB/),
    ).toBeInTheDocument()
    expect(dialog.getByRole('link', { name: 'Download' })).toBeInTheDocument()
    expect(api.requests(contentPath('big'))).toHaveLength(0)
  })

  it('shows a failed load with a retry', async () => {
    const user = userEvent.setup()
    let fail = true
    setup({
      [contentPath('log')]: () =>
        fail
          ? problem(500, 'internal', 'attachment file missing')
          : new Response('INFO start'),
    })
    renderApp(`/runs/${runId}`)
    const dialog = within(await open('harness.log'))

    expect(
      await dialog.findByText(
        'The file could not be loaded: attachment file missing',
      ),
    ).toBeInTheDocument()
    fail = false
    await user.click(dialog.getByRole('button', { name: 'Retry' }))
    expect(await dialog.findByText('INFO start')).toBeInTheDocument()
  })

  it('opens a preview from a link and keeps the case filters', async () => {
    const user = userEvent.setup()
    setup()
    const router = renderApp(
      `/runs/${runId}?device=pixel-8-lab-1&attachment=log`,
    )

    const dialog = await screen.findByRole('dialog', { name: 'harness.log' })
    expect(await within(dialog).findByText(/INFO start/)).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Close' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(currentQuery(router)).toBe('device=pixel-8-lab-1')

    // Opening from the list keeps them too.
    await user.click(await openLink('screenshot.png'))
    expect(currentQuery(router)).toBe('device=pixel-8-lab-1&attachment=img')
  })

  it('ignores an attachment in the URL that the run does not have', async () => {
    setup()
    renderApp(`/runs/${runId}?attachment=elsewhere`)
    await table()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })
})
