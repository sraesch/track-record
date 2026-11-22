import { describe, expect, it } from 'vitest'
import { attachmentKind, mediaType } from './kind'

const kind = (content_type: string, file_name = 'file') =>
  attachmentKind({ content_type, file_name })

describe('attachmentKind', () => {
  it('goes by the content type', () => {
    expect(kind('image/png')).toBe('image')
    expect(kind('image/svg+xml')).toBe('image')
    expect(kind('text/markdown; charset=utf-8')).toBe('markdown')
    expect(kind('application/json')).toBe('json')
    expect(kind('application/vnd.api+json')).toBe('json')
    expect(kind('text/csv')).toBe('text')
    expect(kind('TEXT/PLAIN')).toBe('text')
  })

  it('uses the extension for generic types', () => {
    expect(kind('text/plain', 'summary.md')).toBe('markdown')
    expect(kind('application/octet-stream', 'env.JSON')).toBe('json')
    expect(kind('application/octet-stream', 'harness.log')).toBe('text')
    expect(kind('application/octet-stream', 'trace.bin')).toBeUndefined()
    expect(kind('application/octet-stream', 'screen.png')).toBe('image')
    expect(kind('text/plain', 'screen.png')).toBe('text')
  })

  it('only downloads HTML and types the browser cannot show', () => {
    expect(kind('text/html', 'report.html')).toBeUndefined()
    expect(kind('image/tiff', 'scan.tiff')).toBeUndefined()
    expect(kind('application/pdf', 'report.pdf')).toBeUndefined()
    expect(kind('application/zip', 'trace.zip')).toBeUndefined()
  })
})

describe('mediaType', () => {
  it('drops parameters and case', () => {
    expect(mediaType(' Text/Markdown ; charset=UTF-8')).toBe('text/markdown')
  })
})
