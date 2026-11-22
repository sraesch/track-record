// The derived metric DSL (ADR 0009) as the editor needs it: its functions,
// how names are written and where the backend's diagnostics point. Pure,
// without CodeMirror, so it is unit-tested and shared with the pages.

/** A function of the DSL; `signature` is code, the description is translated. */
export type DslFunction = {
  name: DslFunctionName
  signature: string
}

export type DslFunctionName =
  | 'abs'
  | 'log10'
  | 'sqrt'
  | 'min'
  | 'max'
  | 'clamp'
  | 'mean'
  | 'auc'
  | 'time_to'
  | 'rate'
  | 'cumsum'
  | 'log10_x'
  | 'moving_avg'
  | 'rekey'

/** Every function the backend knows (track-record-core::dsl::check). */
export const dslFunctions: readonly DslFunction[] = [
  { name: 'abs', signature: 'abs(x)' },
  { name: 'log10', signature: 'log10(x)' },
  { name: 'sqrt', signature: 'sqrt(x)' },
  { name: 'min', signature: 'min(a, b, …) · min(s, from=, to=)' },
  { name: 'max', signature: 'max(a, b, …) · max(s, from=, to=)' },
  { name: 'clamp', signature: 'clamp(v, lo, hi)' },
  { name: 'mean', signature: 'mean(s, from=, to=)' },
  { name: 'auc', signature: 'auc(s, from=, to=, interp=)' },
  { name: 'time_to', signature: 'time_to(s, threshold, from=, interp=)' },
  { name: 'rate', signature: 'rate(s)' },
  { name: 'cumsum', signature: 'cumsum(s)' },
  { name: 'log10_x', signature: 'log10_x(s)' },
  { name: 'moving_avg', signature: 'moving_avg(s, window)' },
  { name: 'rekey', signature: 'rekey(s, by, interp=)' },
]

const functionNames: ReadonlySet<string> = new Set(
  dslFunctions.map((f) => f.name),
)

/** Values of the `interp=` argument. */
export const interpolations = ['linear', 'step'] as const

/** Named arguments of the functions. */
export const namedArguments = ['from', 'to', 'interp'] as const

const plainName = /^[A-Za-z_][A-Za-z0-9_]*$/

/** A name as the DSL writes it: plain, or in backticks if it has other characters. */
export function quoteName(name: string): string {
  return plainName.test(name) ? name : `\`${name}\``
}

/** How an expression refers to a derived metric. */
export function derivedReference(name: string): string {
  return `derived.${quoteName(name)}`
}

/**
 * Whether the backend accepts `name` for a derived metric: not empty, no
 * spaces around it, no backtick (it could not be quoted).
 */
export function isValidDerivedName(name: string): boolean {
  return name.trim() !== '' && name === name.trim() && !name.includes('`')
}

/**
 * The index into `text` (UTF-16, as JavaScript strings count) of a UTF-8
 * byte offset, which is how the backend's diagnostics point into the
 * expression. Offsets past the end give the length.
 */
export function byteToIndex(text: string, byte: number): number {
  let bytes = 0
  let i = 0
  while (i < text.length && bytes < byte) {
    const code = text.codePointAt(i) ?? 0
    bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4
    i += code >= 0x10000 ? 2 : 1
  }
  return i
}

/** A problem in an expression, by UTF-16 index. */
export type DslProblem = { from: number; to: number; message: string }

/** The backend's diagnostics (byte offsets) as positions in `text`. */
export function toProblems(
  text: string,
  diagnostics: readonly { start: number; end: number; message: string }[],
): DslProblem[] {
  return diagnostics.map((d) => ({
    from: byteToIndex(text, d.start),
    to: byteToIndex(text, d.end),
    message: d.message,
  }))
}

// Tokens --------------------------------------------------------------------

export type DslTokenType =
  | 'number'
  | 'unit'
  | 'keyword'
  | 'function'
  | 'argument'
  | 'name'
  | 'operator'
  | 'punctuation'
  | 'invalid'

export type DslToken = { type: DslTokenType; from: number; to: number }

const patterns: [DslTokenType | null, RegExp][] = [
  [null, /^\s+/],
  ['number', /^(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/],
  // A unit annotation, `[ms]`; an unclosed one runs to the end.
  ['unit', /^\[[^\]]*\]?/],
  // A quoted name; an unclosed one runs to the end.
  ['name', /^`[^`]*`?/],
  ['name', /^[A-Za-z_][A-Za-z0-9_]*/],
  ['operator', /^[-+*/^=]/],
  ['punctuation', /^[(),.]/],
]

/**
 * Splits an expression into tokens for highlighting. Names are told apart
 * by what follows: `series.`/`derived.` prefixes are keywords, a name
 * before `(` is a function and one before `=` a named argument. Characters
 * the DSL does not know are `invalid`.
 */
export function tokenize(text: string): DslToken[] {
  const tokens: DslToken[] = []
  let at = 0
  while (at < text.length) {
    const rest = text.slice(at)
    let matched = false
    for (const [type, pattern] of patterns) {
      const match = pattern.exec(rest)
      if (!match) continue
      const to = at + match[0].length
      if (type) tokens.push({ type: refine(type, match[0], text, to), from: at, to })
      at = to
      matched = true
      break
    }
    if (!matched) {
      tokens.push({ type: 'invalid', from: at, to: at + 1 })
      at += 1
    }
  }
  return tokens
}

function refine(
  type: DslTokenType,
  word: string,
  text: string,
  end: number,
): DslTokenType {
  if (type !== 'name' || word.startsWith('`')) return type
  const next = text.slice(end).trimStart()[0]
  if ((word === 'series' || word === 'derived') && next === '.') {
    return 'keyword'
  }
  if (next === '(' && functionNames.has(word)) return 'function'
  if (next === '=') return 'argument'
  return type
}

// Completion ----------------------------------------------------------------

export type DslNames = {
  /** Uploaded metrics (scalar and series). */
  metrics: readonly string[]
  /** Saved derived metrics. */
  derived: readonly string[]
}

export type DslCompletionKind =
  | 'function'
  | 'metric'
  | 'series'
  | 'derived'
  | 'keyword'
  | 'value'

export type DslCompletionOption = {
  label: string
  /** Text inserted in place of the typed word. */
  apply: string
  kind: DslCompletionKind
}

export type DslCompletion = {
  /** Start of the word the options replace. */
  from: number
  options: DslCompletionOption[]
}

/**
 * Names as options. In an open backtick the option completes the quoted
 * name (and closes it); otherwise it is quoted where it needs to be.
 */
const names = (
  list: readonly string[],
  kind: DslCompletionKind,
  quoted: boolean,
): DslCompletionOption[] =>
  list.map((name) => ({
    label: name,
    apply: quoted ? `${name}\`` : quoteName(name),
    kind,
  }))

/**
 * What to offer at the end of `before` (the text left of the cursor):
 * after `series.` the uploaded metrics, after `derived.` the derived
 * metrics, after `interp=` its values, in an open backtick the metric
 * names, and for a word the functions, metrics and prefixes. `explicit`
 * (Ctrl+Space) offers them without a word. `from` is where the typed text
 * starts that the options are matched against: after an open backtick.
 */
export function complete(
  before: string,
  names_: DslNames,
  explicit = false,
): DslCompletion | null {
  // Inside a quoted name, the text left of the cursor has an odd number of
  // backticks.
  const quoted = before.split('`').length % 2 === 0
  const quotedWord = quoted ? (/`([^`]*)$/.exec(before)?.[1] ?? '') : ''

  const prefixed =
    /(?<![\w`])(series|derived)\s*\.\s*(`[^`]*|[A-Za-z_][A-Za-z0-9_]*)?$/.exec(
      before,
    )
  if (prefixed) {
    const word = quoted ? quotedWord : (prefixed[2] ?? '')
    return {
      from: before.length - word.length,
      options:
        prefixed[1] === 'series'
          ? names(names_.metrics, 'series', quoted)
          : names(names_.derived, 'derived', quoted),
    }
  }

  if (quoted) {
    return {
      from: before.length - quotedWord.length,
      options: names(names_.metrics, 'metric', true),
    }
  }

  const interp = /interp\s*=\s*([A-Za-z_]*)$/.exec(before)
  if (interp) {
    return {
      from: before.length - (interp[1] ?? '').length,
      options: interpolations.map((value) => ({
        label: value,
        apply: value,
        kind: 'value' as const,
      })),
    }
  }

  // In a unit annotation nothing is offered.
  if (/\[[^\]]*$/.test(before)) return null
  // A word not right after a dot or inside a number.
  const word = /(?<![.\w`])[A-Za-z_][A-Za-z0-9_]*$/.exec(before)?.[0]
  if (word === undefined && !explicit) return null
  return {
    from: before.length - (word?.length ?? 0),
    options: [
      ...dslFunctions.map((f) => ({
        label: f.name,
        apply: `${f.name}(`,
        kind: 'function' as const,
      })),
      { label: 'series.', apply: 'series.', kind: 'keyword' },
      { label: 'derived.', apply: 'derived.', kind: 'keyword' },
      ...names(names_.metrics, 'metric', false),
    ],
  }
}
