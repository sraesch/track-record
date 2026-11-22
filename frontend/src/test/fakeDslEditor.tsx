import type { DslEditorProps } from '../dsl/DslEditor'

// Stands in for the CodeMirror editor in tests (setup.ts): jsdom does not
// lay out text, which CodeMirror needs. A text area with the same label,
// value and description; highlighting and completion are tested on the
// pure functions in dsl/syntax.ts, problems as the page lists them.
export function DslEditor({
  value,
  onChange,
  labelledBy,
  describedBy,
  invalid,
}: DslEditorProps) {
  return (
    <textarea
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      aria-invalid={invalid}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  )
}
