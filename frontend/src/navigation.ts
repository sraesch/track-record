import {
  ArrowLeftRight,
  ChartLine,
  ChartSpline,
  Gauge,
  KeyRound,
  ListChecks,
  type LucideIcon,
  Network,
  Sigma,
  Tag,
} from 'lucide-react'

// The side menu: one entry per top-level view of the web UI (requirements,
// section 4). Routes (routes.tsx) are built from the same list, so menu and
// URLs cannot drift apart.

type NavItem = {
  id: NavItemId
  path: string
  icon: LucideIcon
}

type NavSection = {
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
      { id: 'runs', path: '/runs', icon: ListChecks },
      { id: 'compare', path: '/compare', icon: ArrowLeftRight },
      { id: 'trends', path: '/trends', icon: ChartLine },
      { id: 'series', path: '/series', icon: ChartSpline },
    ],
  },
  {
    id: 'definitions',
    items: [
      { id: 'suites', path: '/suites', icon: Network },
      { id: 'metrics', path: '/metrics', icon: Gauge },
      { id: 'derivedMetrics', path: '/derived-metrics', icon: Sigma },
      { id: 'aliases', path: '/aliases', icon: Tag },
    ],
  },
  {
    id: 'administration',
    items: [{ id: 'tokens', path: '/tokens', icon: KeyRound }],
  },
]

export const navItems: readonly NavItem[] = navSections.flatMap(
  (section) => section.items,
)

/** Where `/` leads. */
export const homePath = '/runs'

/** The About page, opened from the app bar rather than the side menu. */
export const aboutPath = '/about'
