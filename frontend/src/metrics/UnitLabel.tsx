import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import type { Unit } from '../api/client'

type Props = {
  symbol: string
  /** The catalogue by symbol; undefined while it loads. */
  catalogue: ReadonlyMap<string, Unit> | undefined
}

/**
 * A unit symbol. Catalogue units name their unit and quantity on hover;
 * others are marked as custom units, which are not converted (FR-13a).
 */
export function UnitLabel({ symbol, catalogue }: Props) {
  const { t } = useTranslation()
  const unit = catalogue?.get(symbol)
  const text =
    symbol === '' ? (
      <Typography component="span" variant="body2" color="text.secondary">
        {t('metrics.unit.dimensionless')}
      </Typography>
    ) : (
      <Box component="code" sx={{ fontFamily: 'monospace' }}>
        {symbol}
      </Box>
    )

  if (unit) {
    return (
      <Tooltip
        describeChild
        title={t('metrics.unit.known', {
          name: unit.name,
          quantity: unit.quantity,
        })}
      >
        <span>{text}</span>
      </Tooltip>
    )
  }
  return (
    <Box sx={{ display: 'inline-flex', alignItems: 'center', gap: 1 }}>
      {text}
      {catalogue && (
        <Tooltip title={t('metrics.unit.customHint')}>
          <Chip
            size="small"
            variant="outlined"
            color="warning"
            label={t('metrics.unit.custom')}
          />
        </Tooltip>
      )}
    </Box>
  )
}
