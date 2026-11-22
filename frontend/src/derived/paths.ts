import { type DerivedView, derivedViewToParams } from './derivedModel'

/** URL of a view of the derived metrics page (FR-21). */
export function derivedHref(view: DerivedView): string {
  const query = derivedViewToParams(view).toString()
  return query ? `/derived-metrics?${query}` : '/derived-metrics'
}

/** URL of a derived metric, at one version or the latest. */
export const derivedMetricHref = (name: string, version?: number) =>
  derivedHref({ name, version })
