// Small style objects shared by pages.

/** An ID or hash in monospace that may break anywhere. */
export const codeSx = { fontFamily: 'monospace', overflowWrap: 'anywhere' } as const

/** A value with a button next to it, e.g. an ID with its copy button. */
export const inlineRow = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 0.5,
} as const
