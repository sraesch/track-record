import type { ReactElement, SVGProps } from 'react';
import { cx } from '../utils/cx';

/** Names of the icons bundled with Graphite (Lucide-style, 24px grid, 1.5px stroke). */
export type IconName =
  | 'search'
  | 'chevron-down'
  | 'chevron-right'
  | 'plus'
  | 'x'
  | 'check'
  | 'more'
  | 'filter'
  | 'calendar'
  | 'arrow-down'
  | 'arrow-up'
  | 'grid'
  | 'chart'
  | 'database'
  | 'table'
  | 'play'
  | 'refresh'
  | 'download'
  | 'copy'
  | 'trash'
  | 'alert'
  | 'info'
  | 'maximize'
  | 'pin'
  | 'settings'
  | 'eye'
  | 'sun'
  | 'moon';

const paths: Readonly<Record<IconName, ReactElement>> = {
  search: <><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></>,
  'chevron-down': <path d="m6 9 6 6 6-6" />,
  'chevron-right': <path d="m9 6 6 6-6 6" />,
  plus: <path d="M12 5v14M5 12h14" />,
  x: <path d="M18 6 6 18M6 6l12 12" />,
  check: <path d="M20 6 9 17l-5-5" />,
  more: <><circle cx="5" cy="12" r="1" /><circle cx="12" cy="12" r="1" /><circle cx="19" cy="12" r="1" /></>,
  filter: <path d="M3 5h18l-7 8v6l-4 2v-8z" />,
  calendar: <><rect x="3" y="5" width="18" height="16" rx="2" /><path d="M3 10h18M8 3v4M16 3v4" /></>,
  'arrow-down': <path d="M12 5v14M6 13l6 6 6-6" />,
  'arrow-up': <path d="M12 19V5M6 11l6-6 6 6" />,
  grid: <><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" /></>,
  chart: <><path d="M3 3v18h18" /><path d="M8 17v-5M13 17V8M18 17v-9" /></>,
  database: <><ellipse cx="12" cy="5" rx="8" ry="3" /><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" /></>,
  table: <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 10h18M9 10v10" /></>,
  play: <path d="m7 4 13 8-13 8z" />,
  refresh: <path d="M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5" />,
  download: <path d="M12 3v12M7 10l5 5 5-5M4 21h16" />,
  copy: <><rect x="8" y="8" width="13" height="13" rx="2" /><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3" /></>,
  trash: <path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14" />,
  alert: <><circle cx="12" cy="12" r="9" /><path d="M12 8v5M12 16h.01" /></>,
  info: <><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 8h.01" /></>,
  maximize: <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />,
  pin: <path d="M12 17v5M8 3h8l-1 6 3 4H6l3-4z" />,
  settings: <><path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12" /><circle cx="16" cy="6" r="2" /><circle cx="10" cy="12" r="2" /><circle cx="18" cy="18" r="2" /></>,
  eye: <><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" /><circle cx="12" cy="12" r="3" /></>,
  sun: <><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></>,
  moon: <path d="M21 13A9 9 0 1 1 11 3a7 7 0 0 0 10 10z" />,
};
export const iconNames = Object.keys(paths) as readonly IconName[];

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, 'children' | 'name'> {
  name: IconName;
  /** `sm` = 14px (inside small controls and table cells), `md` = 16px (default). */
  size?: 'sm' | 'md';
  /** Accessible label. Without one the icon is decorative and hidden from assistive tech. */
  label?: string;
}

/** Outline icon that inherits `currentColor`. */
export function Icon({ name, size = 'md', label, className, ...rest }: IconProps): ReactElement {
  return (
    <svg
      viewBox="0 0 24 24"
      className={cx('g-icon', size === 'sm' && 'g-icon-sm', className)}
      {...(label !== undefined ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true })}
      {...rest}
    >
      {paths[name]}
    </svg>
  );
}
