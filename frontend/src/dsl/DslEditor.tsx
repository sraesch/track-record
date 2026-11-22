import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
} from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import {
  Compartment,
  EditorState,
  RangeSetBuilder,
  StateEffect,
  StateField,
} from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  EditorView,
  keymap,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'
import Box from '@mui/material/Box'
import { useEffect, useLayoutEffect, useRef } from 'react'
import {
  complete,
  type DslCompletionKind,
  type DslFunctionName,
  type DslNames,
  type DslProblem,
  type DslTokenType,
  dslFunctions,
  tokenize,
} from './syntax'

// The expression editor of the derived metric DSL (ADR 0016): CodeMirror 6
// with the DSL's highlighting, completion of functions and metric names,
// and the backend's diagnostics underlined. Loaded lazily, as its own
// chunk, through ExpressionField.

export type DslEditorProps = {
  value: string
  onChange: (value: string) => void
  /** Problems of `value` (not of an older text), by UTF-16 index. */
  problems: readonly DslProblem[]
  /** Names offered by completion. */
  names: DslNames
  /** Id of the visible label. */
  labelledBy: string
  /** Ids of the help text and the list of problems. */
  describedBy?: string
  invalid?: boolean
  /** Texts of the completion list, translated by the caller. */
  kindLabel: (kind: DslCompletionKind) => string
  functionInfo: (name: DslFunctionName) => string
}

const monospace =
  'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace'

const color = (name: string) => `var(--mui-palette-${name})`

// Colors of the MUI theme as CSS variables (src/theme.ts), so the editor
// follows the color scheme without being rebuilt.
const theme = EditorView.theme({
  '&': {
    fontSize: '0.875rem',
    color: color('text-primary'),
    backgroundColor: 'transparent',
    border: `1px solid rgba(${color('common-onBackgroundChannel')} / 0.23)`,
    borderRadius: '8px',
  },
  '&:hover': { borderColor: color('text-primary') },
  '&.cm-focused': {
    outline: 'none',
    borderColor: color('primary-main'),
    boxShadow: `inset 0 0 0 1px ${color('primary-main')}`,
  },
  '&.dsl-invalid, &.dsl-invalid.cm-focused': {
    borderColor: color('error-main'),
  },
  '.cm-scroller': { fontFamily: monospace, lineHeight: 1.6 },
  '.cm-content': {
    padding: '10px 0',
    minHeight: '4.8em',
    caretColor: color('text-primary'),
  },
  '.cm-line': { padding: '0 14px' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: color('text-primary') },
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground':
    { backgroundColor: `rgba(${color('primary-mainChannel')} / 0.24)` },
  '.cm-tooltip': {
    backgroundColor: color('background-paper'),
    color: color('text-primary'),
    border: `1px solid ${color('divider')}`,
    borderRadius: '4px',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: monospace },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: `rgba(${color('primary-mainChannel')} / 0.16)`,
    color: color('text-primary'),
  },
  '.cm-completionDetail': {
    color: color('text-secondary'),
    fontStyle: 'normal',
    marginLeft: '1em',
  },
  '.cm-completionInfo': {
    fontFamily: 'inherit',
    maxWidth: '320px',
    padding: '4px 8px',
  },
  '.dsl-number': { color: color('info-main') },
  '.dsl-unit': { color: color('success-main') },
  '.dsl-keyword': { color: color('primary-main'), fontWeight: 600 },
  '.dsl-function': { color: color('secondary-main') },
  '.dsl-argument': { color: color('text-secondary'), fontStyle: 'italic' },
  '.dsl-operator': { color: color('text-secondary') },
  '.dsl-invalid': { color: color('error-main') },
  '.dsl-problem': {
    textDecoration: `underline wavy ${color('error-main')}`,
    textUnderlineOffset: '3px',
    textDecorationSkipInk: 'none',
  },
})

const tokenMarks: Record<DslTokenType, Decoration | null> = {
  number: Decoration.mark({ class: 'dsl-number' }),
  unit: Decoration.mark({ class: 'dsl-unit' }),
  keyword: Decoration.mark({ class: 'dsl-keyword' }),
  function: Decoration.mark({ class: 'dsl-function' }),
  argument: Decoration.mark({ class: 'dsl-argument' }),
  operator: Decoration.mark({ class: 'dsl-operator' }),
  invalid: Decoration.mark({ class: 'dsl-invalid' }),
  name: null,
  punctuation: null,
}

function highlightTokens(text: string): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>()
  for (const token of tokenize(text)) {
    const mark = tokenMarks[token.type]
    if (mark) builder.add(token.from, token.to, mark)
  }
  return builder.finish()
}

/** Highlighting with the DSL's tokenizer (syntax.ts), on every change. */
const highlighting = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) {
      this.decorations = highlightTokens(view.state.doc.toString())
    }
    update(update: ViewUpdate) {
      if (update.docChanged) {
        this.decorations = highlightTokens(update.state.doc.toString())
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
)

const setProblems = StateEffect.define<readonly DslProblem[]>()

/** The problems underlined, at least one character each. */
function problemMarks(
  length: number,
  problems: readonly DslProblem[],
): DecorationSet {
  if (length === 0) return Decoration.none
  return Decoration.set(
    problems.map((problem) => {
      let from = Math.min(problem.from, length)
      let to = Math.min(Math.max(problem.to, from), length)
      if (from === to) {
        if (to < length) to += 1
        else from -= 1
      }
      return Decoration.mark({
        class: 'dsl-problem',
        attributes: { title: problem.message },
      }).range(from, to)
    }),
    true,
  )
}

const problemField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(marks, transaction) {
    let next = marks.map(transaction.changes)
    for (const effect of transaction.effects) {
      if (effect.is(setProblems)) {
        next = problemMarks(transaction.state.doc.length, effect.value)
      }
    }
    return next
  },
  provide: (field) => EditorView.decorations.from(field),
})

const completionTypes: Record<DslCompletionKind, string> = {
  function: 'function',
  metric: 'variable',
  series: 'variable',
  derived: 'variable',
  keyword: 'keyword',
  value: 'constant',
}

const isFunction = (name: string): name is DslFunctionName =>
  dslFunctions.some((f) => f.name === name)

function contentAttributes(props: DslEditorProps) {
  const attributes: Record<string, string> = {
    'aria-labelledby': props.labelledBy,
    'aria-invalid': props.invalid ? 'true' : 'false',
    spellcheck: 'false',
    autocapitalize: 'off',
    autocorrect: 'off',
  }
  if (props.describedBy) attributes['aria-describedby'] = props.describedBy
  return [
    EditorView.contentAttributes.of(attributes),
    EditorView.editorAttributes.of({
      class: props.invalid ? 'dsl-invalid' : '',
    }),
  ]
}

export function DslEditor(props: DslEditorProps) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView | null>(null)
  const attributes = useRef(new Compartment())
  // Read by the editor's callbacks, so they always see the latest props
  // without reconfiguring the editor.
  const latest = useRef(props)
  useLayoutEffect(() => {
    latest.current = props
  })

  useEffect(() => {
    const source = (context: CompletionContext): CompletionResult | null => {
      const { names, kindLabel, functionInfo } = latest.current
      const before = context.state.sliceDoc(0, context.pos)
      const result = complete(before, names, context.explicit)
      if (!result) return null
      return {
        from: result.from,
        options: result.options.map(
          (option): Completion => ({
            label: option.label,
            apply: option.apply,
            type: completionTypes[option.kind],
            detail: kindLabel(option.kind),
            info:
              option.kind === 'function' && isFunction(option.label)
                ? `${dslFunctions.find((f) => f.name === option.label)?.signature ?? ''}\n${functionInfo(option.label)}`
                : undefined,
          }),
        ),
        validFor: /^[\w ]*$/,
      }
    }

    const editor = new EditorView({
      parent: host.current ?? undefined,
      state: EditorState.create({
        doc: latest.current.value,
        extensions: [
          history(),
          closeBrackets(),
          autocompletion({ override: [source], icons: false }),
          keymap.of([
            ...closeBracketsKeymap,
            ...completionKeymap,
            ...historyKeymap,
            ...defaultKeymap,
          ]),
          EditorView.lineWrapping,
          highlighting,
          problemField,
          theme,
          attributes.current.of(contentAttributes(latest.current)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              latest.current.onChange(update.state.doc.toString())
            }
          }),
        ],
      }),
    })
    view.current = editor
    return () => {
      editor.destroy()
      view.current = null
    }
  }, [])

  // Text set from outside (a version to start from, a reset).
  const { value, problems, labelledBy, describedBy, invalid } = props
  useEffect(() => {
    const editor = view.current
    if (!editor) return
    const current = editor.state.doc.toString()
    if (value !== current) {
      editor.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      })
    }
  }, [value])

  useEffect(() => {
    view.current?.dispatch({ effects: setProblems.of(problems) })
  }, [problems])

  useEffect(() => {
    view.current?.dispatch({
      effects: attributes.current.reconfigure(
        contentAttributes(latest.current),
      ),
    })
  }, [labelledBy, describedBy, invalid])

  return <Box ref={host} sx={{ minWidth: 0 }} />
}
