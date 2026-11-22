// Splits JSON text into tokens for syntax highlighting. The text is
// rendered as React text nodes, one span per token; nothing is ever parsed
// as HTML.

export type JsonTokenType =
  | 'key'
  | 'string'
  | 'number'
  | 'boolean'
  | 'null'
  | 'punctuation'
  | 'whitespace'
  | 'other'

export type JsonToken = { type: JsonTokenType; text: string }

const tokenPattern =
  /("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|(true|false)|(null)|([{}[\],:])|(\s+)|(.)/gsy

/**
 * Tokens of a JSON text, covering it completely: joining their texts gives
 * the input back. A string followed by a colon is a key. Invalid JSON is
 * tokenized as far as it looks like JSON; the rest is `other`.
 */
export function jsonTokens(text: string): JsonToken[] {
  const tokens: JsonToken[] = []
  tokenPattern.lastIndex = 0
  let match: RegExpExecArray | null
  while ((match = tokenPattern.exec(text)) !== null) {
    const [, string, colon, number, boolean, nul, punctuation, space] = match
    if (string !== undefined) {
      tokens.push({ type: colon ? 'key' : 'string', text: string })
      if (colon) {
        const ws = colon.slice(0, -1)
        if (ws) tokens.push({ type: 'whitespace', text: ws })
        tokens.push({ type: 'punctuation', text: ':' })
      }
    } else if (number !== undefined) tokens.push({ type: 'number', text: number })
    else if (boolean !== undefined) tokens.push({ type: 'boolean', text: boolean })
    else if (nul !== undefined) tokens.push({ type: 'null', text: nul })
    else if (punctuation !== undefined)
      tokens.push({ type: 'punctuation', text: punctuation })
    else if (space !== undefined) tokens.push({ type: 'whitespace', text: space })
    else {
      // Unknown characters come one at a time; keep them together.
      const last = tokens.at(-1)
      if (last?.type === 'other') last.text += match[0]
      else tokens.push({ type: 'other', text: match[0] })
    }
  }
  return tokens
}

/**
 * JSON text pretty-printed with two spaces, or undefined if it is not
 * valid JSON (it is then shown as uploaded).
 */
export function prettyJson(text: string): string | undefined {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return undefined
  }
}
