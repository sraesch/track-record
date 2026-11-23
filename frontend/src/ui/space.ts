/** Spacing steps of Graphite's scale: 1 = 4px, 2 = 8px, 3 = 12px, 4 = 16px, 5 = 20px, 6 = 24px, 8 = 32px. */
export type Space = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 8 | 12

/** CSS length of a spacing step, e.g. for `style={{ marginTop: space(2) }}`. */
export const space = (step: Space) => (step === 0 ? 0 : `var(--space-${step})`)
