import Box from '@mui/material/Box'
import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'
import { type ReactNode, useId } from 'react'

type Props = {
  title: string
  children: ReactNode
  /** Shown to the right of the title, e.g. a count or actions. */
  actions?: ReactNode
  /** No padding around the content, for a table that fills the section. */
  flush?: boolean
}

/** A titled block of a page, e.g. one part of a run's detail. */
export function Section({ title, children, actions, flush }: Props) {
  const titleId = useId()
  return (
    <Paper
      component="section"
      variant="outlined"
      aria-labelledby={titleId}
      sx={{ minWidth: 0, overflow: 'hidden' }}
    >
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          flexWrap: 'wrap',
          gap: 1,
          px: 2,
          pt: 1.5,
          pb: flush ? 1.5 : 1,
          minHeight: 48,
        }}
      >
        <Typography
          id={titleId}
          variant="subtitle1"
          component="h2"
          sx={{ fontWeight: 500, flexGrow: 1 }}
        >
          {title}
        </Typography>
        {actions}
      </Box>
      <Box sx={flush ? undefined : { px: 2, pb: 2 }}>{children}</Box>
    </Paper>
  )
}
