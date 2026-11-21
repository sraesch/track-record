import { QueryClient } from '@tanstack/react-query'

// Views are live (FR-21): data is refetched when it is older than
// `staleTime` and the window regains focus or the view mounts again.
export function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        retry: 1,
      },
    },
  })
}
