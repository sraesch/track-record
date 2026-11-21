import CssBaseline from '@mui/material/CssBaseline'
import { ThemeProvider } from '@mui/material/styles'
import { type QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { type createBrowserRouter, RouterProvider } from 'react-router'
import { createQueryClient } from './api/queryClient'
import { theme } from './theme'

type Props = {
  /** A browser router in the app, a memory router in tests. */
  router: ReturnType<typeof createBrowserRouter>
  /** Tests pass their own, without retries. */
  queryClient?: QueryClient
}

function App({ router, queryClient }: Props) {
  const [defaultClient] = useState(() => createQueryClient())
  return (
    <QueryClientProvider client={queryClient ?? defaultClient}>
      <ThemeProvider theme={theme}>
        <CssBaseline enableColorScheme />
        <RouterProvider router={router} />
      </ThemeProvider>
    </QueryClientProvider>
  )
}

export default App
