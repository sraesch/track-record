import {
  Ban,
  CircleCheck,
  CircleStop,
  CircleX,
  LoaderCircle,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { RunStatus } from '../api/client'
import { Chip, type ChipTone, Tooltip } from '../ui'

const look: Record<RunStatus, { tone: ChipTone; icon: typeof CircleCheck }> = {
  running: { tone: 'info', icon: LoaderCircle },
  completed: { tone: 'success', icon: CircleCheck },
  failed: { tone: 'danger', icon: CircleX },
  aborted: { tone: 'warning', icon: CircleStop },
}

/** Run status (FR-2) with a color and an icon, so it does not rest on color alone. */
export function RunStatusChip({ status }: { status: RunStatus }) {
  const { t } = useTranslation()
  const { tone, icon } = look[status]
  return (
    <Chip tone={tone} icon={icon}>
      {t(`runs.status.${status}`)}
    </Chip>
  )
}

/** Marks an invalidated run (FR-9); the reason is in the tooltip. */
export function InvalidChip({ reason }: { reason?: string | null }) {
  const { t } = useTranslation()
  const chip = (
    <Chip icon={Ban} outlined>
      {t('runs.invalid')}
    </Chip>
  )
  // The tooltip also labels the chip, so screen readers hear the reason.
  return reason ? (
    <Tooltip title={t('runs.invalidReason', { reason })} labels>
      {chip}
    </Tooltip>
  ) : (
    chip
  )
}
