import { useTranslation } from 'react-i18next'
import { Annotations } from '../../annotations/Annotations'
import { useTestCaseAnnotations } from '../../annotations/queries'

/**
 * The annotations of the test case of a case execution. They are on the
 * case (scenario version, assets, parameters), so every execution of it,
 * in every run, shows the same ones.
 */
export function CaseAnnotations({ testCaseId }: { testCaseId: number }) {
  const { t } = useTranslation()
  const annotations = useTestCaseAnnotations(testCaseId)
  return (
    <Annotations
      title={t('caseExecution.annotations.title')}
      description={t('caseExecution.annotations.description')}
      annotations={annotations.data}
      target={{ test_case_id: testCaseId }}
      loadError={
        annotations.isError
          ? {
              message: annotations.error.message,
              onRetry: () => annotations.refetch(),
            }
          : undefined
      }
    />
  )
}
