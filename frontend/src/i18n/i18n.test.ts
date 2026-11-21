import { describe, expect, it } from 'vitest'
import i18n, { resources, supportedLanguages } from './index'

// Flattens nested translation objects to dotted keys.
function keys(value: unknown, prefix = ''): string[] {
  if (typeof value !== 'object' || value === null) return [prefix]
  return Object.entries(value).flatMap(([k, v]) =>
    keys(v, prefix ? `${prefix}.${k}` : k),
  )
}

describe('i18n', () => {
  it('starts in English', () => {
    expect(i18n.language).toBe('en')
    expect(document.documentElement.lang).toBe('en')
  })

  it('resolves keys to text', () => {
    expect(i18n.t('app.name')).toBe('Track Record')
  })

  it('has the same keys in every language', () => {
    const reference = keys(resources.en).sort()
    const byLanguage = Object.fromEntries(
      supportedLanguages.map((lng) => [lng, keys(resources[lng]).sort()]),
    )
    const expected = Object.fromEntries(
      supportedLanguages.map((lng) => [lng, reference]),
    )
    expect(byLanguage).toEqual(expected)
  })
})
