import Chip from '@mui/material/Chip'
import Tooltip from '@mui/material/Tooltip'
import { useTranslation } from 'react-i18next'

/** Marks a warmup sample, which aggregation leaves out by default (FR-12). */
export function WarmupChip() {
  const { t } = useTranslation()
  return (
    <Tooltip describeChild title={t('caseExecution.warmupHint')}>
      <Chip
        size="small"
        variant="outlined"
        label={t('caseExecution.warmup')}
        sx={{ ml: 1 }}
      />
    </Tooltip>
  )
}
