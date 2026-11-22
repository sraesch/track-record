import Box from '@mui/material/Box'
import Typography from '@mui/material/Typography'
import { Fragment, type ReactNode } from 'react'

export type Fact = {
  label: string
  value: ReactNode
  /** The label in monospace, e.g. a key from uploaded data. */
  code?: boolean
}

/**
 * Label/value pairs as a description list. Where there is room, the pairs
 * are laid out in two columns.
 */
export function Facts({ items }: { items: readonly Fact[] }) {
  return (
    <Box sx={{ containerType: 'inline-size' }}>
      <Box
        component="dl"
        sx={{
          m: 0,
          display: 'grid',
          gridTemplateColumns: 'max-content minmax(0, 1fr)',
          columnGap: 2,
          rowGap: 1,
          alignItems: 'baseline',
          '@container (min-width: 640px)': {
            gridTemplateColumns:
              'max-content minmax(0, 1fr) max-content minmax(0, 1fr)',
            columnGap: 3,
          },
        }}
      >
        {items.map(({ label, value, code }) => (
          <Fragment key={label}>
            <Typography
              component="dt"
              variant="body2"
              color="text.secondary"
              sx={code ? { fontFamily: 'monospace' } : undefined}
            >
              {label}
            </Typography>
            <Box
              component="dd"
              sx={{ m: 0, minWidth: 0, overflowWrap: 'anywhere' }}
            >
              {value}
            </Box>
          </Fragment>
        ))}
      </Box>
    </Box>
  )
}
