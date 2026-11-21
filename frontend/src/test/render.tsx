import { QueryClient } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { createMemoryRouter } from 'react-router'
import App from '../App'
import { routes } from '../routes'

/** Renders the whole app at a path; returns the router to inspect the URL. */
export function renderApp(path: string) {
  const router = createMemoryRouter(routes, { initialEntries: [path] })
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  render(<App router={router} queryClient={queryClient} />)
  return router
}

/** The current URL query of the router, e.g. "status=failed&tag=x". */
export function currentQuery(router: ReturnType<typeof renderApp>) {
  return router.state.location.search.replace(/^\?/, '')
}
