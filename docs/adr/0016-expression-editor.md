# ADR 0016: Expression editor for the derived metric DSL

**Status:** proposed · **Date:** 2026-11-22

## Context

Derived metrics are written in a small DSL (FR-17a,
[ADR 0009](0009-derived-metric-dsl-and-analysis.md)). The web UI edits them
on the derived metrics page and will reuse the editor wherever an analysis
view takes an expression. Writing them in a plain text field works, but:

- Names come in three kinds (`load_time`, `series.fps`, `derived.x`), some
  need backticks, and the functions take named arguments (`from=`,
  `interp=`). Completion of names and functions avoids most typos.
- The backend validates while typing (`POST /derived-metrics/validate`) and
  points at problems by byte offset. They should be marked in the text, not
  only listed.
- Highlighting tells metric names, functions, units and numbers apart at a
  glance.

Candidates:

| | Bundle (gzip) | Completion | Marks in the text | Accessibility | Notes |
|---|---|---|---|---|---|
| **CodeMirror 6** (state, view, commands, autocomplete) | ~99 kB, own chunk | built in, any source | decorations | `contenteditable` with `role="textbox"`, screen reader support, no Tab trap by default | modular, maintained; themes as CSS, so MUI's CSS variables work |
| Monaco | ~1 MB+ plus workers | built in | built in | good | far too large for one-line expressions; web workers in the build |
| MUI `TextField` + own overlay | 0 | by hand (popper, keyboard handling) | by hand (mirror element under a transparent textarea) | native | fragile caret/overlay alignment; completion and its keyboard handling would be ours to maintain |

## Decision

- **CodeMirror 6**: `@codemirror/state`, `view`, `commands` and
  `autocomplete` (which brings `language` with it). No Lezer grammar: the
  DSL is highlighted with its own tokenizer (`src/dsl/syntax.ts`), which
  also drives completion. That keeps the grammar knowledge in one small,
  unit-tested, CodeMirror-free module.
- The editor (`src/dsl/DslEditor.tsx`) is loaded lazily through
  `ExpressionField` as the `editor` chunk (`vite.config.ts`), like the
  charts (ADR 0015). If the chunk cannot be loaded, a plain monospace
  text field takes its place, so expressions can still be edited.
- Colors are the MUI theme's CSS variables (`var(--mui-palette-…)`), so
  the editor follows the color scheme without being rebuilt.
- Problems from the backend are byte offsets into the UTF-8 expression;
  `toProblems()` converts them to string indices. They are underlined
  only while they belong to the text on screen, and listed below the
  editor, which the editor names in `aria-describedby`.
- Tests replace the editor with a text area (`src/test/fakeDslEditor.tsx`):
  jsdom does not lay out text, which CodeMirror needs. Highlighting and
  completion are tested on the pure functions.
- Dependencies are resolved to releases at least two weeks old
  (`npm install --before=<date>`).

## Consequences

- About 99 kB gzipped, loaded only on pages with an expression editor.
- The tokenizer must follow the backend's lexer and function list
  (`track-record-core::dsl`); a new function needs an entry in
  `dslFunctions` and a translated description.
- Completion offers uploaded metric names for both scalars and series: the
  metrics endpoint does not say which kind a metric is.
