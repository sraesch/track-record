import { useTranslation } from 'react-i18next'
import { matchPath, NavLink, useLocation } from 'react-router'
import { navSections } from '../navigation'

type Props = {
  /** Called after an item is chosen, e.g. to close the overlay menu. */
  onNavigate?: () => void
}

/** The side menu, grouped in sections. The current view is marked. */
export function NavMenu({ onNavigate }: Props) {
  const { t } = useTranslation()
  const { pathname } = useLocation()

  return navSections.map((section) => (
    <div key={section.id} className="tr-nav-section">
      <div id={`nav-section-${section.id}`} className="g-menu-group">
        {t(`nav.sections.${section.id}`)}
      </div>
      <ul aria-labelledby={`nav-section-${section.id}`}>
        {section.items.map((item) => {
          const Icon = item.icon
          const label = t(`nav.items.${item.id}`)
          // A view stays marked on its sub-pages (/runs/:runId).
          const current =
            matchPath({ path: item.path, end: false }, pathname) !== null
          return (
            <li key={item.id}>
              <NavLink
                to={item.path}
                className="g-nav-item"
                // The label is hidden in the collapsed rail; the title names it there.
                title={label}
                aria-current={current ? 'page' : undefined}
                onClick={onNavigate}
              >
                <Icon size={16} aria-hidden />
                <span className="g-nav-label">{label}</span>
              </NavLink>
            </li>
          )
        })}
      </ul>
    </div>
  ))
}
