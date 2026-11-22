import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import Button from '@mui/material/Button'
import { useTranslation } from 'react-i18next'
import { Link, useParams } from 'react-router'
import { runPath } from '../runs/paths'
import { NotBuilt } from './PlaceholderPage'
import { PageHeader } from './PageHeader'

/**
 * One case execution (FR-21: every case execution has a URL), reached from
 * its run. Not built yet.
 */
export function CaseExecutionPage() {
  const { t } = useTranslation()
  const { runId = '' } = useParams()
  return (
    <>
      <PageHeader
        title={t('caseExecution.title')}
        description={t('caseExecution.description')}
        before={
          <Button
            component={Link}
            to={runPath(runId)}
            size="small"
            startIcon={<ArrowBackIcon />}
            sx={{ mb: 1, ml: -1 }}
          >
            {t('caseExecution.back')}
          </Button>
        }
      />
      <NotBuilt />
    </>
  )
}
