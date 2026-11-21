import ConstructionIcon from '@mui/icons-material/Construction'
import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import type { NavItemId } from '../navigation'
import { PageHeader } from './PageHeader'

/** Stand-in for a top-level view that is not built yet. */
export function PlaceholderPage({ id }: { id: NavItemId }) {
  const { t } = useTranslation()
  return (
    <>
      <PageHeader
        title={t(`nav.items.${id}`)}
        description={t(`pages.${id}.description`)}
      />
      <NotBuilt />
    </>
  )
}

export function NotBuilt() {
  const { t } = useTranslation()
  return (
    <Paper
      variant="outlined"
      sx={{
        p: 4,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 1,
        color: 'text.secondary',
        borderStyle: 'dashed',
      }}
    >
      <ConstructionIcon />
      <Typography>{t('placeholder.notBuilt')}</Typography>
    </Paper>
  )
}
