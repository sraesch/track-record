import { useTranslation } from 'react-i18next'
import { useLocation } from 'react-router'
import { homePath } from '../navigation'
import { LinkButton } from '../ui'
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
      <LinkButton to={homePath} variant="primary">
        {t('pages.notFound.home')}
      </LinkButton>
    </>
  )
}
