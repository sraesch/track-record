import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { RunDetail } from '../../api/client'
import { Tags } from '../../tags/Tags'
import { useAddRunTag, useRemoveRunTag } from './mutations'

/** Tags of a run; a tag opens the run list filtered by it. */
export function RunTags({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  return (
    <Tags
      title={t('runDetail.tags.title')}
      tags={run.tags}
      add={useAddRunTag(run.id)}
      remove={useRemoveRunTag(run.id)}
      open={{
        label: (tag) => t('runDetail.tags.show', { tag }),
        onOpen: (tag) => navigate(`/runs?${new URLSearchParams({ tag })}`),
      }}
    />
  )
}
