import { useTranslation } from 'react-i18next'
import { Annotations } from '../../annotations/Annotations'
import type { RunDetail } from '../../api/client'

/** The annotations of a run; they come with the run. */
export function RunAnnotations({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  return (
    <Annotations
      title={t('runDetail.annotations.title')}
      annotations={run.annotations}
      target={{ run_id: run.id }}
    />
  )
}
