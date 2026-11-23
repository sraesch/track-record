import { useTranslation } from 'react-i18next'
import { Chip, Tooltip } from '../../ui'
import './case.css'

/** Marks a warmup sample, which aggregation leaves out by default (FR-12). */
export function WarmupChip() {
  const { t } = useTranslation()
  return (
    <Tooltip title={t('caseExecution.warmupHint')}>
      <Chip outlined className="tr-case-warmup">
        {t('caseExecution.warmup')}
      </Chip>
    </Tooltip>
  )
}
