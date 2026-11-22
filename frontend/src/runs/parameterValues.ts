import { useTranslation } from 'react-i18next'
import type { TestCaseSummary } from '../api/client'
import { formatJsonValue } from '../format'
import { jsonObject } from './detail/json'

/** A parameter's value after aliases (FR-14), with its unit (FR-13a). */
export type ParameterValue = {
  name: string
  /** Formatted, with the unit if it has one. */
  label: string
  /** The uploaded value, if an alias changed it. */
  uploaded?: string
}

/** The parameters of a test case after aliases, sorted by name. */
export function useParameterValues(testCase: TestCaseSummary): ParameterValue[] {
  const { t } = useTranslation()
  const resolved = jsonObject(testCase.resolved_parameters)
  const uploaded = jsonObject(testCase.parameters)
  const units = jsonObject(testCase.parameter_units)
  return Object.keys(resolved)
    .sort()
    .map((name) => {
      const unit = units[name]
      const value = formatJsonValue(resolved[name])
      // JSON comparison: an alias may also change the type (FR-14).
      const aliased =
        name in uploaded &&
        JSON.stringify(uploaded[name]) !== JSON.stringify(resolved[name])
      return {
        name,
        label:
          typeof unit === 'string' && unit
            ? t('common.valueWithUnit', { value, unit })
            : value,
        uploaded: aliased ? formatJsonValue(uploaded[name]) : undefined,
      }
    })
}
