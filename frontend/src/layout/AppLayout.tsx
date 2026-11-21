import MenuIcon from '@mui/icons-material/Menu'
import MenuOpenIcon from '@mui/icons-material/MenuOpen'
import AppBar from '@mui/material/AppBar'
import Box from '@mui/material/Box'
import Drawer from '@mui/material/Drawer'
import IconButton from '@mui/material/IconButton'
import { useTheme } from '@mui/material/styles'
import Toolbar from '@mui/material/Toolbar'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import useMediaQuery from '@mui/material/useMediaQuery'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, Outlet } from 'react-router'
import { homePath } from '../navigation'
import { drawerWidth } from '../theme'
import { ColorModeMenu } from './ColorModeMenu'
import { NavMenu } from './NavMenu'

/**
 * The frame of every page: app bar on top, side menu on the left, the page
 * in the middle. On wide screens the menu is docked and can be hidden; on
 * narrow screens it slides in over the page.
 */
export function AppLayout() {
  const { t } = useTranslation()
  const theme = useTheme()
  const wide = useMediaQuery(theme.breakpoints.up('md'))
  const [dockedOpen, setDockedOpen] = useState(true)
  const [overlayOpen, setOverlayOpen] = useState(false)

  const menuOpen = wide ? dockedOpen : overlayOpen
  const toggleMenu = () =>
    wide ? setDockedOpen((open) => !open) : setOverlayOpen((open) => !open)

  return (
    <Box sx={{ display: 'flex', minHeight: '100vh' }}>
      <AppBar
        position="fixed"
        elevation={0}
        sx={{
          zIndex: (th) => th.zIndex.drawer + 1,
          borderBottom: 1,
          borderColor: 'divider',
        }}
      >
        <Toolbar>
          <Tooltip title={menuOpen ? t('nav.close') : t('nav.open')}>
            <IconButton
              color="inherit"
              edge="start"
              aria-label={menuOpen ? t('nav.close') : t('nav.open')}
              aria-expanded={menuOpen}
              aria-controls="side-menu"
              onClick={toggleMenu}
              sx={{ mr: 1.5 }}
            >
              {menuOpen ? <MenuOpenIcon /> : <MenuIcon />}
            </IconButton>
          </Tooltip>
          <Typography
            variant="h6"
            component={Link}
            to={homePath}
            aria-label={t('nav.home')}
            noWrap
            sx={{ color: 'inherit', textDecoration: 'none', fontWeight: 600 }}
          >
            {t('app.name')}
          </Typography>
          <Box sx={{ flexGrow: 1 }} />
          <ColorModeMenu />
        </Toolbar>
      </AppBar>

      <Drawer
        id="side-menu"
        variant={wide ? 'persistent' : 'temporary'}
        open={menuOpen}
        onClose={() => setOverlayOpen(false)}
        ModalProps={{ keepMounted: true }}
        sx={{
          width: wide && dockedOpen ? drawerWidth : 0,
          flexShrink: 0,
          transition: (th) => th.transitions.create('width'),
          '& .MuiDrawer-paper': { width: drawerWidth, boxSizing: 'border-box' },
        }}
      >
        <Toolbar />
        <NavMenu onNavigate={wide ? undefined : () => setOverlayOpen(false)} />
      </Drawer>

      <Box
        component="main"
        sx={{ flexGrow: 1, minWidth: 0, p: { xs: 2, sm: 3 } }}
      >
        <Toolbar />
        <Outlet />
      </Box>
    </Box>
  )
}
