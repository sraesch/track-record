import BlockIcon from '@mui/icons-material/Block'
import CancelIcon from '@mui/icons-material/Cancel'
import CheckCircleIcon from '@mui/icons-material/CheckCircle'
import PendingIcon from '@mui/icons-material/Pending'
import StopCircleIcon from '@mui/icons-material/StopCircle'
import Chip, { type ChipProps } from '@mui/material/Chip'
import Tooltip from '@mui/material/Tooltip'
import { useTranslation } from 'react-i18next'
import type { RunStatus } from '../api/client'

const look: Record<
  RunStatus,
  { color: ChipProps['color']; icon: typeof CheckCircleIcon }
> = {
  running: { color: 'info', icon: PendingIcon },
  completed: { color: 'success', icon: CheckCircleIcon },
  failed: { color: 'error', icon: CancelIcon },
  aborted: { color: 'warning', icon: StopCircleIcon },
}

/** Run status (FR-2) as a colored chip. */
export function RunStatusChip({ status }: { status: RunStatus }) {
  const { t } = useTranslation()
  const { color, icon: Icon } = look[status]
  return (
    <Chip
      size="small"
      variant="outlined"
      color={color}
      icon={<Icon />}
      label={t(`runs.status.${status}`)}
    />
  )
}

/** Marks an invalidated run (FR-9); the reason is in the tooltip. */
export function InvalidChip({ reason }: { reason?: string | null }) {
  const { t } = useTranslation()
  const chip = (
    <Chip
      size="small"
      variant="outlined"
      icon={<BlockIcon />}
      label={t('runs.invalid')}
      aria-label={
        reason ? t('runs.invalidReason', { reason }) : t('runs.invalid')
      }
    />
  )
  return reason ? (
    <Tooltip title={t('runs.invalidReason', { reason })}>{chip}</Tooltip>
  ) : (
    chip
  )
}
