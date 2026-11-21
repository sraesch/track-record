import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'
import type { ReactNode } from 'react'

type Props = {
  children: ReactNode
  icon?: ReactNode
  /** Dashed border, for something that is not there yet. */
  dashed?: boolean
}

/** Stands in for content that is empty or missing. */
export function EmptyState({ children, icon, dashed }: Props) {
  return (
    <Paper
      variant="outlined"
      sx={{
        p: 4,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 1,
        textAlign: 'center',
        color: 'text.secondary',
        borderStyle: dashed ? 'dashed' : undefined,
      }}
    >
      {icon}
      <Typography>{children}</Typography>
    </Paper>
  )
}
