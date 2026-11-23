import type { SampleDetail } from '../../api/client'
import './case.css'

// Shared by the measurement and series tables of a case execution.

/** Samples in repetition order. */
export const byRepetition = (samples: readonly SampleDetail[]) =>
  [...samples].sort((a, b) => a.repetition - b.repetition)

/** Dims the cells of a warmup sample's row (on a `<tr>` of a `g-table`). */
export const warmupRowClass = 'tr-case-warmup-row'
