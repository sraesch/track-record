import { useTranslation } from 'react-i18next'
import { type AliasValue, typeOf, valueText } from './aliasModel'

/**
 * Formats alias values as text. With `typed` (parameter values, which keep
 * their JSON type), the type is added, so 10 and "10" read differently.
 */
export function useValueLabel() {
  const { t } = useTranslation()
  return (value: AliasValue, typed: boolean) =>
    typed
      ? t('aliases.valueWithType', {
          value: valueText(value),
          type: t(`aliases.type.${typeOf(value)}`),
        })
      : valueText(value)
}
