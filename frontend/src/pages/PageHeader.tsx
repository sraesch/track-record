import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import './pages.css'

type Props = {
  title: string
  description?: string
  /** Shown above the title, e.g. a back link. */
  before?: ReactNode
  /** Shown to the right of the title, e.g. actions. */
  actions?: ReactNode
}

/** Title block of a page; also sets the browser tab title. */
export function PageHeader({ title, description, before, actions }: Props) {
  const { t } = useTranslation()
  return (
    <div className="tr-page-header">
      {/* React puts it into <head>: "<title> · Track Record". */}
      <title>{`${title} · ${t('app.name')}`}</title>
      {before}
      {/* Actions move below a title that leaves them no room. */}
      <div className="tr-page-title-row">
        <h1 className="title-1 ui-heading ui-grow">{title}</h1>
        {actions}
      </div>
      {description && <p className="tr-page-description">{description}</p>}
    </div>
  )
}
