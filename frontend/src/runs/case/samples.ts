import type { SampleDetail } from '../../api/client'

// Shared by the measurement and series tables of a case execution.

/** Samples in repetition order. */
export const byRepetition = (samples: readonly SampleDetail[]) =>
  [...samples].sort((a, b) => a.repetition - b.repetition)

/** Dims the cells of a warmup sample's row. */
export const warmupRowSx = { '& td': { color: 'text.secondary' } } as const
