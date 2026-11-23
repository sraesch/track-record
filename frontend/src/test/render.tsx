import { render } from '@testing-library/react'
import { createMemoryRouter } from 'react-router'
import { vi } from 'vitest'
import App from '../App'
import { createQueryClient } from '../api/queryClient'
import { routes } from '../routes'
import { resetDocument } from './reset'
import { setViewportWidth } from './viewport'

/** Renders the whole app at a path; returns the router to inspect the URL. */
export function renderApp(path: string) {
  const router = createMemoryRouter(routes, { initialEntries: [path] })
  const queryClient = createQueryClient({ retry: false })
  render(<App router={router} queryClient={queryClient} />)
  return router
}

/** The current URL query of the router, e.g. "status=failed&tag=x". */
export function currentQuery(router: ReturnType<typeof renderApp>) {
  return router.state.location.search.replace(/^\?/, '')
}

// The first render in a test file is the slowest: the JIT compiler has not
// seen React yet. Render the shell once while the file loads, so that cost (0.2–0.4 s,
// several times that on a loaded machine) does not count against the first
// test's timeout. Requests never answer; the run list stays loading.
function warmUp() {
  setViewportWidth(1280)
  vi.stubGlobal('fetch', () => new Promise(() => {}))
  renderApp('/runs')
  resetDocument()
}

warmUp()
