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
    <div style={{ padding: 'var(--space-6)' }}>
      <PageHeader
        title={t('pages.error.title')}
        description={t('pages.error.description')}
      />
      {/* A full page load: the app state may be broken. */}
      <a
        href={homePath}
        className="g-btn g-btn-primary"
        style={{ textDecoration: 'none' }}
      >
        {t('pages.error.home')}
      </a>
    </div>
  )
}
