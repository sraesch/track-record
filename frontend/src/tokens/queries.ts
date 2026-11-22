import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, type CreateToken, unwrap } from '../api/client'

export const tokenKeys = {
  all: ['tokens'] as const,
}

/** All API tokens without their secrets, newest first; revoked ones too. */
export function useTokens() {
  return useQuery({
    queryKey: tokenKeys.all,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET('/ui/api/v1/tokens', { signal })),
  })
}

/**
 * Creates a token. The answer is the only place its secret ever appears:
 * the server keeps a hash. The caller shows it once; it is not put into
 * the query cache, and the mutation is dropped as soon as nothing uses it.
 */
export function useCreateToken() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateToken) =>
      unwrap(await api.POST('/ui/api/v1/tokens', { body })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: tokenKeys.all }),
    gcTime: 0,
  })
}

/** Revokes a token for good. The server keeps it, marked as revoked. */
export function useRevokeToken() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (id: string) =>
      unwrap(
        await api.POST('/ui/api/v1/tokens/{token_id}/revoke', {
          params: { path: { token_id: id } },
        }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: tokenKeys.all }),
  })
}
