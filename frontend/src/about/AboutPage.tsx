import { lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorBoundary } from '../pages/ErrorBoundary'
import { PageHeader } from '../pages/PageHeader'
import { Alert, Button, ProgressBar } from '../ui'

// The changelog comes with the Markdown renderer, which is large and has its
// own chunk (vite.config.ts): both are loaded when this page is opened.
const Changelog = lazy(() =>
  import('./Changelog').then((module) => ({ default: module.Changelog })),
)

/** About the app: what it is for and what changed in each version. */
export function AboutPage() {
  const { t } = useTranslation()
  return (
    <>
      <PageHeader
        title={t('about.title')}
        description={t('about.description')}
      />
      {/* Short lines are easier to read on a wide screen. */}
      <div style={{ maxWidth: '80ch' }}>
        <ErrorBoundary
          fallback={() => (
            <Alert
              tone="error"
              action={
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => window.location.reload()}
                >
                  {t('about.reload')}
                </Button>
              }
            >
              {t('about.loadFailed')}
            </Alert>
          )}
        >
          <Suspense fallback={<ProgressBar />}>
            <Changelog />
          </Suspense>
        </ErrorBoundary>
      </div>
    </>
  )
}
