import Tooltip from '@mui/material/Tooltip'
import { formatDateTime, formatTimestamp } from '../format'

/** A timestamp; the full one with seconds and time zone in the tooltip. */
export function Time({ iso }: { iso: string }) {
  return (
    <Tooltip describeChild title={formatTimestamp(iso)}>
      <time dateTime={iso}>{formatDateTime(iso)}</time>
    </Tooltip>
  )
}
