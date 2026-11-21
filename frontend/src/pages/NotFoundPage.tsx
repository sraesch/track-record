import Button from '@mui/material/Button'
import { useTranslation } from 'react-i18next'
import { Link, useLocation } from 'react-router'
import { homePath } from '../navigation'
import { PageHeader } from './PageHeader'

export function NotFoundPage() {
  const { t } = useTranslation()
  const { pathname } = useLocation()
  return (
    <>
      <PageHeader
        title={t('pages.notFound.title')}
        description={t('pages.notFound.description', { path: pathname })}
      />
      <Button component={Link} to={homePath} variant="contained">
        {t('pages.notFound.home')}
      </Button>
    </>
  )
}
