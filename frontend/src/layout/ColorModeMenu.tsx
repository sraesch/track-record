import {
  Menu,
  type MenuEntry,
  type ThemePreference,
  useTheme,
} from '@graphite/react'
import { useTranslation } from 'react-i18next'
import { useColorMode } from '../ui'

const modes: readonly ThemePreference[] = ['system', 'light', 'dark']

/** Title bar menu to follow the system color scheme or force light or dark. */
export function ColorModeMenu() {
  const { t } = useTranslation()
  const { mode, setMode } = useColorMode()
  const { resolved } = useTheme()
  const items: MenuEntry[] = modes.map((option) => ({
    id: option,
    type: 'checkbox',
    label: t(`colorMode.${option}`),
    checked: option === mode,
    onCheckedChange: () => setMode(option),
  }))
  return (
    <Menu
      label={t('colorMode.label')}
      icon={resolved === 'light' ? 'sun' : 'moon'}
      iconOnly
      align="end"
      items={items}
    />
  )
}
