import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import LinearProgress from '@mui/material/LinearProgress'
import { lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorBoundary } from '../pages/ErrorBoundary'
import { PageHeader } from '../pages/PageHeader'

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
      <PageHeader title={t('about.title')} description={t('about.description')} />
      {/* Short lines are easier to read on a wide screen. */}
      <Box sx={{ maxWidth: '80ch' }}>
        <ErrorBoundary
          fallback={() => (
            <Alert
              severity="error"
              action={
                <Button
                  color="inherit"
                  size="small"
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
          <Suspense
            fallback={<LinearProgress aria-label={t('common.loading')} />}
          >
            <Changelog />
          </Suspense>
        </ErrorBoundary>
      </Box>
    </>
  )
}
