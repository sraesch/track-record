import Box from '@mui/material/Box'
import type { SxProps, Theme } from '@mui/material/styles'
import type { ReactNode } from 'react'

/** Preformatted text in a monospace block; long lines wrap. */
export function CodeBlock({
  children,
  sx,
}: {
  children: ReactNode
  sx?: SxProps<Theme>
}) {
  return (
    <Box
      component="pre"
      sx={[
        {
          m: 0,
          p: 2,
          borderRadius: 1,
          bgcolor: 'action.hover',
          fontFamily: 'monospace',
          fontSize: '0.8125rem',
          lineHeight: 1.6,
          whiteSpace: 'pre-wrap',
          overflowWrap: 'anywhere',
        },
        ...(Array.isArray(sx) ? sx : [sx]),
      ]}
    >
      {children}
    </Box>
  )
}

/** A text attachment (a log, CSV, …) as uploaded. */
export function TextView({ text }: { text: string }) {
  return <CodeBlock>{text}</CodeBlock>
}
