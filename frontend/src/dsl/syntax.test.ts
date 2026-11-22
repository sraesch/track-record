import { describe, expect, it } from 'vitest'
import {
  byteToIndex,
  complete,
  derivedReference,
  isValidDerivedName,
  quoteName,
  toProblems,
  tokenize,
} from './syntax'

const names = {
  metrics: ['load_time', 'load time (cold)'],
  derived: ['throughput', 'time to 90'],
}

const labels = (before: string, explicit = false) =>
  complete(before, names, explicit)?.options.map((o) => o.label)

describe('names', () => {
  it('quotes names that are not plain identifiers', () => {
    expect(quoteName('load_time')).toBe('load_time')
    expect(quoteName('load time (cold)')).toBe('`load time (cold)`')
    expect(quoteName('9lives')).toBe('`9lives`')
    expect(derivedReference('time to 90')).toBe('derived.`time to 90`')
  })

  it('accepts the names the backend accepts', () => {
    expect(isValidDerivedName('throughput')).toBe(true)
    expect(isValidDerivedName('time to 90 %')).toBe(true)
    expect(isValidDerivedName('')).toBe(false)
    expect(isValidDerivedName(' padded')).toBe(false)
    expect(isValidDerivedName('back`tick')).toBe(false)
  })
})

describe('byteToIndex', () => {
  it('maps UTF-8 byte offsets to string indices', () => {
    expect(byteToIndex('a + b', 4)).toBe(4)
    // "é" is 2 bytes, 1 UTF-16 unit.
    expect(byteToIndex('`é` + x', 4)).toBe(3)
    // "€" is 3 bytes, 1 unit; "😀" 4 bytes, 2 units.
    expect(byteToIndex('€x', 3)).toBe(1)
    expect(byteToIndex('😀x', 4)).toBe(2)
    expect(byteToIndex('ab', 99)).toBe(2)
  })

  it('turns diagnostics into problems in the text', () => {
    // y is the 11th character, but two 2-byte characters come before it.
    expect(
      toProblems('`größe` + y', [{ start: 12, end: 13, message: 'unknown y' }]),
    ).toEqual([{ from: 10, to: 11, message: 'unknown y' }])
  })
})

describe('tokenize', () => {
  const types = (text: string) =>
    tokenize(text).map((t) => [text.slice(t.from, t.to), t.type])

  it('tells the kinds of tokens apart', () => {
    expect(
      types('mean(series.fps, from=derived.t0) * 500[ms] + `a b` ^ 2'),
    ).toEqual([
      ['mean', 'function'],
      ['(', 'punctuation'],
      ['series', 'keyword'],
      ['.', 'punctuation'],
      ['fps', 'name'],
      [',', 'punctuation'],
      ['from', 'argument'],
      ['=', 'operator'],
      ['derived', 'keyword'],
      ['.', 'punctuation'],
      ['t0', 'name'],
      [')', 'punctuation'],
      ['*', 'operator'],
      ['500', 'number'],
      ['[ms]', 'unit'],
      ['+', 'operator'],
      ['`a b`', 'name'],
      ['^', 'operator'],
      ['2', 'number'],
    ])
  })

  it('treats unknown functions as names and unknown characters as invalid', () => {
    expect(types('nope(x) $ 1.5e-3')).toEqual([
      ['nope', 'name'],
      ['(', 'punctuation'],
      ['x', 'name'],
      [')', 'punctuation'],
      ['$', 'invalid'],
      ['1.5e-3', 'number'],
    ])
  })

  it('runs unclosed quotes and units to the end', () => {
    expect(types('`open name')).toEqual([['`open name', 'name']])
    expect(types('1[m')).toEqual([
      ['1', 'number'],
      ['[m', 'unit'],
    ])
  })
})

describe('complete', () => {
  it('offers functions, prefixes and metrics for a word', () => {
    const result = complete('2 * lo', names)
    expect(result?.from).toBe(4)
    expect(result?.options.map((o) => o.label)).toEqual([
      'abs',
      'log10',
      'sqrt',
      'min',
      'max',
      'clamp',
      'mean',
      'auc',
      'time_to',
      'rate',
      'cumsum',
      'log10_x',
      'moving_avg',
      'rekey',
      'series.',
      'derived.',
      'load_time',
      'load time (cold)',
    ])
    const option = (label: string) =>
      result?.options.find((o) => o.label === label)?.apply
    expect(option('mean')).toBe('mean(')
    expect(option('load time (cold)')).toBe('`load time (cold)`')
  })

  it('offers metrics after series. and derived metrics after derived.', () => {
    expect(labels('mean(series.')).toEqual(['load_time', 'load time (cold)'])
    expect(complete('mean(series.lo', names)?.from).toBe(12)
    expect(labels('1 + derived.th')).toEqual(['throughput', 'time to 90'])
    expect(
      complete('derived.', names)?.options.find((o) => o.label === 'time to 90')
        ?.apply,
    ).toBe('`time to 90`')
  })

  it('completes a quoted name after its backtick', () => {
    const result = complete('x + `load t', names)
    expect(result?.from).toBe(5)
    expect(result?.options.map((o) => o.apply)).toEqual([
      'load_time`',
      'load time (cold)`',
    ])
    expect(complete('derived.`time', names)?.options[1]?.apply).toBe(
      'time to 90`',
    )
    // A closed quote is not open any more.
    expect(complete('`a b` + ', names)).toBeNull()
  })

  it('offers the interpolations after interp=', () => {
    expect(labels('auc(series.fps, interp=')).toEqual(['linear', 'step'])
    expect(complete('auc(s, interp=st', names)?.from).toBe(14)
  })

  it('offers nothing inside numbers and units, and everything on request', () => {
    expect(complete('1e', names)).toBeNull()
    expect(complete('500[m', names)).toBeNull()
    expect(complete('a + ', names)).toBeNull()
    expect(labels('a + ', true)).toContain('series.')
  })
})
