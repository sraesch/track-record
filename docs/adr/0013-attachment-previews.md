# ADR 0013: Attachment previews in the web UI

**Status:** proposed · **Date:** 2026-11-22

## Context

Runs, case executions and samples carry attachments: screenshots, logs,
reports, environment dumps (FR-5). The web UI shows a run with its
attachments (requirements, section 4). Downloading every file to look at it
is slow work for a screenshot or a short report.

The backend serves an attachment's content only as a download
(`Content-Disposition: attachment`, `Content-Security-Policy: sandbox`,
`nosniff`), and uploaded HTML must never be rendered inline
([ADR 0003](0003-attachment-storage.md)). Uploads come from the harness
(`write` tokens), so their content is not fully trusted.

## Decision

- **The app renders previews itself**, from the content URL on the UI port;
  the backend is unchanged. A file opens in a dialog with its name, type,
  size and a download button. Every file can still be downloaded.
- **What is shown how** (by content type; for generic types such as
  `application/octet-stream` or `text/plain`, by file extension):
  - **Images** (PNG, JPEG, GIF, WebP, AVIF, BMP, SVG) in an `<img>`.
    Scripts in an SVG never run in an `<img>`.
  - **Markdown** with `react-markdown` and `remark-gfm` (tables, task
    lists). HTML in the Markdown is dropped, link targets are sanitized by
    react-markdown, links open in a new tab with `noopener noreferrer`, and
    images are not loaded (a report must not make the browser fetch from
    elsewhere); they are offered as links.
  - **JSON** pretty-printed and highlighted by a small tokenizer of our own
    (`src/attachments/jsonTokens.ts`): spans with text, never HTML. Invalid
    JSON is shown as uploaded, with a note.
  - **Other text** (`text/*`, logs) as uploaded, in a monospace block.
  - **Everything else, HTML in particular,** is only downloaded.
- **Limits:** text previews up to 1 MB; larger files are only downloaded.
  JSON over 250,000 characters is shown without highlighting.
- **Thumbnails:** image attachments get a small preview in the attachment
  list, loaded lazily. It is the full image scaled down; images over 5 MB
  get an icon instead. The backend makes no thumbnails.
- **Dialog or own page:** a click on a file opens it in a dialog over the
  run's page; the open dialog is part of that page's URL
  (`/runs/:runId?attachment=<id>`, FR-21), next to its filters, and the
  back button closes it. Every file can also be opened in a new tab on its
  own page, `/runs/:runId/attachments/:attachmentId` (a deep link to the
  file, FR-21): with a button in the list and in the dialog, and by opening
  the file's link in a new tab (middle click, Ctrl/Cmd+click, context
  menu), since the link leads there. On its own page an image is shown at
  its size, not fitted to the window.
- **Bundle:** the Markdown renderer (~50 kB gzipped) is loaded only when a
  Markdown file is opened, as its own chunk (`vite.config.ts`).
- **Failures stay in the dialog:** a preview that cannot render, e.g.
  because its lazily loaded code is gone after a new release, shows an
  error with a reload button instead of breaking the page.

## Consequences

- New dependencies: `react-markdown` and `remark-gfm`, with the unified and
  micromark packages they build on.
- A run with many large screenshots loads them in full for the thumbnails,
  as they scroll into view.
- The case execution page can reuse the preview (`src/attachments/`) when
  it is built.

## When to revisit

Thumbnails make the attachment list slow (then the backend should store
small versions at upload), or other types (PDF, CSV as a table, traces)
need a preview.
