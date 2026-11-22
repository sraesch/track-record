import Box from '@mui/material/Box'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import { type AliasValue, typeOf, valueText } from './aliasModel'

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
    <Box
      component="span"
      sx={{ display: 'inline-flex', gap: 0.75, alignItems: 'baseline' }}
    >
      <Box component="span" sx={{ fontWeight: 500, whiteSpace: 'nowrap' }}>
        {valueText(value)}
      </Box>
      {typed && (
        <Typography component="span" variant="caption" color="text.secondary">
          {t(`aliases.type.${typeOf(value)}`)}
        </Typography>
      )}
    </Box>
  )
}
