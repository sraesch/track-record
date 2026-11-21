import { vi } from 'vitest'

// A fake UI API for tests: stubs the global fetch, answers by path and
// records every request.
//
//   const api = mockApi({ '/ui/api/v1/runs': () => ({ runs: [] }) })
//   ...
//   expect(api.requests('/ui/api/v1/runs')[0].searchParams.getAll('status'))

type Handler = (url: URL, request: Request) => unknown | Response

export type MockApi = {
  /** URLs of the requests to `path`, in order. */
  requests: (path: string) => URL[]
}

/** Empty answers for every list endpoint the app calls on its own. */
const defaults: Record<string, Handler> = {
  '/ui/api/v1/runs': () => ({ runs: [], next_cursor: null }),
  '/ui/api/v1/suites': () => [],
  '/ui/api/v1/client-devices': () => [],
  '/ui/api/v1/sut-versions': () => [],
}

export function mockApi(handlers: Record<string, Handler> = {}): MockApi {
  const all = { ...defaults, ...handlers }
  const seen: URL[] = []
  vi.stubGlobal('fetch', async (input: Request) => {
    const url = new URL(input.url)
    seen.push(url)
    const handler = all[url.pathname]
    if (!handler) {
      return problem(404, 'not_found', `no mock for ${url.pathname}`)
    }
    const result = await handler(url, input)
    return result instanceof Response
      ? result
      : new Response(JSON.stringify(result), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
  })
  return {
    requests: (path) => seen.filter((url) => url.pathname === path),
  }
}

/** An RFC 9457 error response, as the backend sends it. */
export function problem(status: number, code: string, detail: string) {
  return new Response(
    JSON.stringify({ title: 'Error', status, code, detail }),
    { status, headers: { 'content-type': 'application/problem+json' } },
  )
}
