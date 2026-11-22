import { describe, expect, it } from 'vitest'
import { formatBytes, formatJsonValue, formatNumber } from './format'

describe('format', () => {
  it('formats file sizes in decimal units', () => {
    expect(formatBytes(0)).toBe('0 bytes')
    expect(formatBytes(1)).toBe('1 byte')
    expect(formatBytes(999)).toBe('999 bytes')
    expect(formatBytes(1536)).toBe('1.5 kB')
    expect(formatBytes(2_400_000)).toBe('2.4 MB')
    expect(formatBytes(5e15)).toBe('5,000 TB')
  })

  it('formats numbers without floating point noise', () => {
    expect(formatNumber(0.1 + 0.2)).toBe('0.3')
    expect(formatNumber(1234567.25)).toBe('1,234,567.25')
  })

  it('formats JSON values by their type', () => {
    expect(formatJsonValue(20)).toBe('20')
    expect(formatJsonValue('draco')).toBe('draco')
    expect(formatJsonValue(true)).toBe('true')
    expect(formatJsonValue(null)).toBe('null')
    expect(formatJsonValue(['a', 1])).toBe('["a",1]')
  })
})
