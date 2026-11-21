import BrightnessAutoIcon from '@mui/icons-material/BrightnessAuto'
import DarkModeIcon from '@mui/icons-material/DarkMode'
import LightModeIcon from '@mui/icons-material/LightMode'
import IconButton from '@mui/material/IconButton'
import ListItemIcon from '@mui/material/ListItemIcon'
import ListItemText from '@mui/material/ListItemText'
import Menu from '@mui/material/Menu'
import MenuItem from '@mui/material/MenuItem'
import { useColorScheme } from '@mui/material/styles'
import Tooltip from '@mui/material/Tooltip'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

const modes = [
  { mode: 'system', icon: BrightnessAutoIcon },
  { mode: 'light', icon: LightModeIcon },
  { mode: 'dark', icon: DarkModeIcon },
] as const

/** App bar button to follow the system color scheme or force light/dark. */
export function ColorModeMenu() {
  const { t } = useTranslation()
  const { mode, setMode } = useColorScheme()
  const [anchor, setAnchor] = useState<HTMLElement | null>(null)

  const current = modes.find((m) => m.mode === mode) ?? modes[0]
  const CurrentIcon = current.icon

  return (
    <>
      {/* The tooltip also labels the button. */}
      <Tooltip title={t('colorMode.label')}>
        <IconButton
          color="inherit"
          aria-haspopup="menu"
          aria-controls={anchor ? 'color-mode-menu' : undefined}
          aria-expanded={anchor ? 'true' : undefined}
          onClick={(event) => setAnchor(event.currentTarget)}
        >
          <CurrentIcon />
        </IconButton>
      </Tooltip>
      <Menu
        id="color-mode-menu"
        anchorEl={anchor}
        open={anchor !== null}
        onClose={() => setAnchor(null)}
      >
        {modes.map(({ mode: option, icon: Icon }) => (
          <MenuItem
            key={option}
            selected={option === current.mode}
            onClick={() => {
              setMode(option)
              setAnchor(null)
            }}
          >
            <ListItemIcon>
              <Icon fontSize="small" />
            </ListItemIcon>
            <ListItemText primary={t(`colorMode.${option}`)} />
          </MenuItem>
        ))}
      </Menu>
    </>
  )
}
