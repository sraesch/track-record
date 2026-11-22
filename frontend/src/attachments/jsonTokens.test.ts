import { describe, expect, it } from 'vitest'
import { jsonTokens, prettyJson } from './jsonTokens'

const types = (text: string) =>
  jsonTokens(text)
    .filter((t) => t.type !== 'whitespace')
    .map((t) => `${t.type}:${t.text}`)

describe('jsonTokens', () => {
  it('tells keys from string values', () => {
    expect(types('{"name" : "hub", "tags": ["a"]}')).toEqual([
      'punctuation:{',
      'key:"name"',
      'punctuation::',
      'string:"hub"',
      'punctuation:,',
      'key:"tags"',
      'punctuation::',
      'punctuation:[',
      'string:"a"',
      'punctuation:]',
      'punctuation:}',
    ])
  })

  it('finds numbers, booleans, null and escaped quotes', () => {
    expect(types('[-1.5e3, 0, true, false, null, "say \\"hi\\":"]')).toEqual([
      'punctuation:[',
      'number:-1.5e3',
      'punctuation:,',
      'number:0',
      'punctuation:,',
      'boolean:true',
      'punctuation:,',
      'boolean:false',
      'punctuation:,',
      'null:null',
      'punctuation:,',
      'string:"say \\"hi\\":"',
      'punctuation:]',
    ])
  })

  it('covers the whole text, also when it is not JSON', () => {
    for (const text of [
      '{\n  "a": [1, 2]\n}',
      '{"a": NaN, "b": undefined}',
      '"unterminated',
      '',
    ]) {
      expect(
        jsonTokens(text)
          .map((t) => t.text)
          .join(''),
      ).toBe(text)
    }
    expect(types('{"a": NaN}')).toContain('other:NaN')
  })
})

describe('prettyJson', () => {
  it('indents valid JSON and rejects anything else', () => {
    expect(prettyJson('{"a":[1,{"b":null}]}')).toBe(
      '{\n  "a": [\n    1,\n    {\n      "b": null\n    }\n  ]\n}',
    )
    expect(prettyJson('{"a": NaN}')).toBeUndefined()
  })
})
