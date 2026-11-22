import type { ApiToken, TokenScope } from '../api/client'

// API tokens (NFR-2) authenticate machine clients on `/api` and `/mcp`.
// A token has one scope; `annotate` and `write` include `read` but not each
// other. Revoked tokens are kept, marked by `revoked_at`.

/** The scopes, from the least to the most a token may do. */
export const tokenScopes: readonly TokenScope[] = ['read', 'annotate', 'write']

export const isRevoked = (token: ApiToken) => token.revoked_at != null

export type TokensView = {
  /** Revoked tokens are listed too. */
  revoked: boolean
}

export function parseTokensView(params: URLSearchParams): TokensView {
  return { revoked: params.get('revoked') === '1' }
}

export function tokensViewToParams(view: TokensView): URLSearchParams {
  const params = new URLSearchParams()
  if (view.revoked) params.set('revoked', '1')
  return params
}

/** The tokens the view lists, in the order of the server (newest first). */
export function shownTokens(
  tokens: readonly ApiToken[],
  view: TokensView,
): ApiToken[] {
  return tokens.filter((token) => view.revoked || !isRevoked(token))
}

/**
 * True if a token that is not revoked has this name. Names need not be
 * unique, but two active tokens with one name cannot be told apart.
 */
export function nameInUse(tokens: readonly ApiToken[], name: string): boolean {
  const wanted = name.trim()
  return (
    wanted !== '' &&
    tokens.some((token) => !isRevoked(token) && token.name === wanted)
  )
}
