import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { CaseExecutionDetail } from '../../api/client'
import { Tags } from '../../tags/Tags'
import { caseFiltersToParams } from '../detail/caseFilters'
import { runPath } from '../paths'
import { useAddTestCaseTag, useRemoveTestCaseTag } from './mutations'

/**
 * The tags of the test case of a case execution. Like its annotations, they
 * are on the case, so every execution of it, in every run, shows them. A
 * tag opens the run's case executions searched for it.
 */
export function CaseTags({
  caseExecution: ce,
}: {
  caseExecution: CaseExecutionDetail
}) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const testCaseId = ce.test_case.id
  return (
    <Tags
      title={t('caseExecution.tags.title')}
      description={t('caseExecution.tags.description')}
      tags={ce.test_case.tags}
      add={useAddTestCaseTag(testCaseId)}
      remove={useRemoveTestCaseTag(testCaseId)}
      open={{
        label: (tag) => t('caseExecution.tags.show', { tag }),
        onOpen: (tag) =>
          navigate(
            `${runPath(ce.run_id)}?${caseFiltersToParams({ search: tag, superseded: false })}`,
          ),
      }}
    />
  )
}
