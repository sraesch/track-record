import { type DerivedView, derivedViewToParams } from './derivedModel'

/** URL of a view of the derived metrics page (FR-21). */
export function derivedHref(view: DerivedView): string {
  const query = derivedViewToParams(view).toString()
  return query ? `/derived-metrics?${query}` : '/derived-metrics'
}

/** URL of a derived metric. */
export const derivedMetricHref = (name: string) => derivedHref({ name })
