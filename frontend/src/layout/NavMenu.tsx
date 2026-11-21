import List from '@mui/material/List'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemIcon from '@mui/material/ListItemIcon'
import ListItemText from '@mui/material/ListItemText'
import ListSubheader from '@mui/material/ListSubheader'
import { useTranslation } from 'react-i18next'
import { matchPath, NavLink, useLocation } from 'react-router'
import { navSections } from '../navigation'

type Props = {
  /** Called after an item is chosen, e.g. to close the mobile drawer. */
  onNavigate?: () => void
}

/** The side menu, grouped in sections. The current view is highlighted. */
export function NavMenu({ onNavigate }: Props) {
  const { t } = useTranslation()
  const { pathname } = useLocation()

  return (
    <nav aria-label={t('nav.label')}>
      {navSections.map((section) => (
        <List
          key={section.id}
          dense
          subheader={
            <ListSubheader
              id={`nav-section-${section.id}`}
              disableSticky
              sx={{ bgcolor: 'transparent', lineHeight: '2.5rem' }}
            >
              {t(`nav.sections.${section.id}`)}
            </ListSubheader>
          }
          aria-labelledby={`nav-section-${section.id}`}
        >
          {section.items.map((item) => {
            const Icon = item.icon
            // A view stays selected on its sub-pages (/runs/:runId).
            const selected =
              matchPath({ path: item.path, end: false }, pathname) !== null
            return (
              <ListItemButton
                key={item.id}
                component={NavLink}
                to={item.path}
                selected={selected}
                onClick={onNavigate}
                sx={{ mx: 1, borderRadius: 1 }}
              >
                <ListItemIcon sx={{ minWidth: 40 }}>
                  <Icon fontSize="small" />
                </ListItemIcon>
                <ListItemText primary={t(`nav.items.${item.id}`)} />
              </ListItemButton>
            )
          })}
        </List>
      ))}
    </nav>
  )
}
