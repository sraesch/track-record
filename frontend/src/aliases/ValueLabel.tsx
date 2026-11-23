import { useTranslation } from 'react-i18next'
import { type AliasValue, typeOf, valueText } from './aliasModel'
import './aliases.css'

/** An alias value in the table: the value, and its type if `typed`. */
export function ValueLabel({
  value,
  typed,
}: {
  value: AliasValue
  typed: boolean
}) {
  const { t } = useTranslation()
  return (
    <span className="tr-aliases-value">
      <span className="body-medium ui-nowrap">{valueText(value)}</span>
      {typed && (
        <span className="small ui-secondary">
          {t(`aliases.type.${typeOf(value)}`)}
        </span>
      )}
    </span>
  )
}
