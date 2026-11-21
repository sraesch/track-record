import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import Button from '@mui/material/Button'
import { useTranslation } from 'react-i18next'
import { Link, useParams } from 'react-router'
import { NotBuilt } from './PlaceholderPage'
import { PageHeader } from './PageHeader'

/** One run (FR-21: every run has a URL). Not built yet. */
export function RunDetailPage() {
  const { t } = useTranslation()
  const { runId = '' } = useParams()
  return (
    <>
      <PageHeader
        title={t('pages.runDetail.title', { runId })}
        description={t('pages.runDetail.description')}
        before={
          <Button
            component={Link}
            to="/runs"
            size="small"
            startIcon={<ArrowBackIcon />}
            sx={{ mb: 1, ml: -1 }}
          >
            {t('pages.runDetail.back')}
          </Button>
        }
      />
      <NotBuilt />
    </>
  )
}
