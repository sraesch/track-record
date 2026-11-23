import { type QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { LucideProvider } from 'lucide-react'
import { useState } from 'react'
import type { createBrowserRouter } from 'react-router'
// From `react-router/dom`, so navigations can use `flushSync`.
import { RouterProvider } from 'react-router/dom'
import { createQueryClient } from './api/queryClient'
import { ColorModeProvider } from './ui'

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
      <ColorModeProvider>
        {/* Graphite's icon style: 16px, 1.5px strokes (ADR 0019). */}
        <LucideProvider size={16} strokeWidth={1.5}>
          <RouterProvider router={router} />
        </LucideProvider>
      </ColorModeProvider>
    </QueryClientProvider>
  )
}

export default App
