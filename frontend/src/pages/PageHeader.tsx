import Box from '@mui/material/Box'
import Typography from '@mui/material/Typography'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

type Props = {
  title: string
  description?: string
  /** Shown above the title, e.g. a back link. */
  before?: ReactNode
  /** Shown to the right of the title, e.g. actions. */
  actions?: ReactNode
}

/** Title block of a page; also sets the browser tab title. */
export function PageHeader({ title, description, before, actions }: Props) {
  const { t } = useTranslation()
  return (
    <Box sx={{ mb: 3 }}>
      {/* React puts it into <head>: "<title> · Track Record". */}
      <title>{`${title} · ${t('app.name')}`}</title>
      {before}
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
        <Typography variant="h5" component="h1" sx={{ flexGrow: 1 }}>
          {title}
        </Typography>
        {actions}
      </Box>
      {description && (
        <Typography color="text.secondary" sx={{ mt: 0.5, maxWidth: '72ch' }}>
          {description}
        </Typography>
      )}
    </Box>
  )
}
