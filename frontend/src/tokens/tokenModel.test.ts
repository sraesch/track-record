import { describe, expect, it } from 'vitest'
import type { ApiToken } from '../api/client'
import {
  nameInUse,
  parseTokensView,
  shownTokens,
  tokensViewToParams,
} from './tokenModel'

const token = (name: string, revoked = false): ApiToken => ({
  id: `id-${name}`,
  name,
  scope: 'read',
  created_at: '2026-11-01T08:00:00Z',
  last_used_at: null,
  revoked_at: revoked ? '2026-11-02T08:00:00Z' : null,
})

const tokens = [token('harness'), token('old harness', true), token('mcp')]

describe('tokens view', () => {
  it('reads and writes the URL', () => {
    expect(parseTokensView(new URLSearchParams(''))).toEqual({
      revoked: false,
    })
    expect(parseTokensView(new URLSearchParams('revoked=1'))).toEqual({
      revoked: true,
    })
    expect(tokensViewToParams({ revoked: false }).toString()).toBe('')
    expect(tokensViewToParams({ revoked: true }).toString()).toBe('revoked=1')
  })

  it('lists revoked tokens only on request, in the order of the server', () => {
    expect(shownTokens(tokens, { revoked: false }).map((t) => t.name)).toEqual(
      ['harness', 'mcp'],
    )
    expect(shownTokens(tokens, { revoked: true })).toEqual(tokens)
  })
})

describe('nameInUse', () => {
  it('is true for the name of a token that is not revoked', () => {
    expect(nameInUse(tokens, 'harness')).toBe(true)
    expect(nameInUse(tokens, '  mcp ')).toBe(true)
    expect(nameInUse(tokens, 'old harness')).toBe(false)
    expect(nameInUse(tokens, 'new')).toBe(false)
    expect(nameInUse(tokens, '')).toBe(false)
  })
})
