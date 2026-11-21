import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import { useTranslation } from 'react-i18next'
import { useRouteError } from 'react-router'
import { homePath } from '../navigation'
import { PageHeader } from './PageHeader'

/** Shown when a page throws while rendering or loading. */
export function ErrorPage() {
  const { t } = useTranslation()
  const error = useRouteError()
  if (import.meta.env.DEV) console.error(error)
  return (
    <Box sx={{ p: 3 }}>
      <PageHeader
        title={t('pages.error.title')}
        description={t('pages.error.description')}
      />
      {/* A full page load: the app state may be broken. */}
      <Button href={homePath} variant="contained">
        {t('pages.error.home')}
      </Button>
    </Box>
  )
}
