import type { SvgIconComponent } from '@mui/icons-material'
import CompareArrowsIcon from '@mui/icons-material/CompareArrows'
import FunctionsIcon from '@mui/icons-material/Functions'
import KeyIcon from '@mui/icons-material/Key'
import LabelIcon from '@mui/icons-material/Label'
import ListAltIcon from '@mui/icons-material/ListAlt'
import ShowChartIcon from '@mui/icons-material/ShowChart'
import SpeedIcon from '@mui/icons-material/Speed'
import StackedLineChartIcon from '@mui/icons-material/StackedLineChart'
import AccountTreeIcon from '@mui/icons-material/AccountTree'

// The side menu: one entry per top-level view of the web UI (requirements,
// section 4). Routes (routes.tsx) are built from the same list, so menu and
// URLs cannot drift apart.

export type NavItem = {
  id: NavItemId
  path: string
  icon: SvgIconComponent
}

export type NavSection = {
  id: 'analysis' | 'definitions' | 'administration'
  items: readonly NavItem[]
}

export type NavItemId =
  | 'runs'
  | 'compare'
  | 'trends'
  | 'series'
  | 'suites'
  | 'metrics'
  | 'derivedMetrics'
  | 'aliases'
  | 'tokens'

export const navSections: readonly NavSection[] = [
  {
    id: 'analysis',
    items: [
      { id: 'runs', path: '/runs', icon: ListAltIcon },
      { id: 'compare', path: '/compare', icon: CompareArrowsIcon },
      { id: 'trends', path: '/trends', icon: ShowChartIcon },
      { id: 'series', path: '/series', icon: StackedLineChartIcon },
    ],
  },
  {
    id: 'definitions',
    items: [
      { id: 'suites', path: '/suites', icon: AccountTreeIcon },
      { id: 'metrics', path: '/metrics', icon: SpeedIcon },
      { id: 'derivedMetrics', path: '/derived-metrics', icon: FunctionsIcon },
      { id: 'aliases', path: '/aliases', icon: LabelIcon },
    ],
  },
  {
    id: 'administration',
    items: [{ id: 'tokens', path: '/tokens', icon: KeyIcon }],
  },
]

export const navItems: readonly NavItem[] = navSections.flatMap(
  (section) => section.items,
)

/** Where `/` leads. */
export const homePath = '/runs'
