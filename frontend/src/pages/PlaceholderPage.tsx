import { Construction } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { NavItemId } from '../navigation'
import { EmptyState } from './EmptyState'
import { PageHeader } from './PageHeader'

/** Stand-in for a top-level view that is not built yet. */
export function PlaceholderPage({ id }: { id: NavItemId }) {
  const { t } = useTranslation()
  return (
    <>
      <PageHeader
        title={t(`nav.items.${id}`)}
        description={t(`pages.${id}.description`)}
      />
      <NotBuilt />
    </>
  )
}

export function NotBuilt() {
  const { t } = useTranslation()
  return (
    <EmptyState icon={<Construction size={24} aria-hidden />} dashed>
      {t('placeholder.notBuilt')}
    </EmptyState>
  )
}
