import { cx } from '@graphite/react'
import { Info, PanelLeftClose, PanelLeftOpen } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, Outlet } from 'react-router'
import { aboutPath, homePath } from '../navigation'
import { breakpoints, IconButton, IconLinkButton, useMediaQuery } from '../ui'
import { ColorModeMenu } from './ColorModeMenu'
import './layout.css'
import { NavMenu } from './NavMenu'

const appIconUrl = `${import.meta.env.BASE_URL}favicon.svg`

/**
 * The frame of every page (Graphite's app shell): title bar on top, side
 * menu on the left, the page in the middle. On wide screens the menu is
 * docked and can be collapsed to a rail of icons; on narrow screens it
 * slides in over the page and closes after navigating.
 */
export function AppLayout() {
  const { t } = useTranslation()
  const wide = useMediaQuery(`(min-width:${breakpoints.md}px)`)
  const [docked, setDocked] = useState(true)
  const [overlay, setOverlay] = useState(false)

  const expanded = wide ? docked : overlay
  const showMenu = wide || overlay
  const toggleLabel = wide
    ? t(docked ? 'nav.collapse' : 'nav.expand')
    : t(overlay ? 'nav.close' : 'nav.open')

  return (
    <div
      className={cx(
        'g-shell',
        'g-shell-nostatus',
        'tr-shell',
        wide && !docked && 'g-shell-collapsed',
      )}
    >
      <header className="g-titlebar">
        <IconButton
          icon={expanded ? PanelLeftClose : PanelLeftOpen}
          label={toggleLabel}
          aria-expanded={expanded}
          aria-controls="side-menu"
          onClick={() => (wide ? setDocked((d) => !d) : setOverlay((o) => !o))}
        />
        <Link to={homePath} className="tr-home" aria-label={t('nav.home')}>
          {/* The same file as the favicon; the link's label names it. */}
          <img src={appIconUrl} alt="" />
          {t('app.name')}
        </Link>
        <div className="g-titlebar-actions">
          <IconLinkButton icon={Info} label={t('about.label')} to={aboutPath} />
          <ColorModeMenu />
        </div>
      </header>

      {showMenu && (
        <nav
          id="side-menu"
          className="g-sidebar tr-sidebar"
          aria-label={t('nav.label')}
        >
          {/* The overlay closes after navigating. */}
          <NavMenu onNavigate={wide ? undefined : () => setOverlay(false)} />
        </nav>
      )}
      {!wide && overlay && (
        <button
          type="button"
          className="tr-scrim"
          aria-label={t('nav.close')}
          tabIndex={-1}
          onClick={() => setOverlay(false)}
        />
      )}

      <main className="g-main tr-main">
        <div className="tr-page">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
