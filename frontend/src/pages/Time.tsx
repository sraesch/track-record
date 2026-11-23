import { formatDateTime, formatTimestamp } from '../format'
import { Tooltip } from '../ui'

/** A timestamp; the full one with seconds and time zone in the tooltip. */
export function Time({ iso }: { iso: string }) {
  return (
    <Tooltip title={formatTimestamp(iso)}>
      <time dateTime={iso}>{formatDateTime(iso)}</time>
    </Tooltip>
  )
}
