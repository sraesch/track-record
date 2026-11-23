import type { AnchorHTMLAttributes, ReactElement, ReactNode } from 'react';
import { cx } from '../utils/cx';
import { Icon, type IconName } from './Icon';

export interface AppShellProps {
  /** Left side of the title bar, usually `Breadcrumbs`. */
  title: ReactNode;
  /** Right side of the title bar: command search, view actions. */
  titlebarActions?: ReactNode;
  /** Sidebar content: `NavGroup`s and `NavItem`s. */
  sidebar: ReactNode;
  /** Status bar content (connection, timing, version). Omit to hide the bar. */
  statusbar?: ReactNode;
  /** Accessible name of the sidebar navigation. Default "Main". */
  sidebarLabel?: string;
  /** Icon-only sidebar rail. */
  sidebarCollapsed?: boolean;
  /** Main area: toolbar, FilterBar, then content. */
  children: ReactNode;
  className?: string;
}

/** The window layout: title bar, sidebar, main area, status bar. Give it a height (e.g. 100vh). */
export function AppShell({ title, titlebarActions, sidebar, sidebarLabel = 'Main', statusbar, sidebarCollapsed = false, children, className }: AppShellProps): ReactElement {
  return (
    <div className={cx('g-shell', sidebarCollapsed && 'g-shell-collapsed', statusbar === undefined && 'g-shell-nostatus', className)}>
      <header className="g-titlebar">
        {title}
        {titlebarActions !== undefined && <div className="g-titlebar-actions">{titlebarActions}</div>}
      </header>
      <nav className="g-sidebar" aria-label={sidebarLabel}>{sidebar}</nav>
      <main className="g-main">{children}</main>
      {statusbar !== undefined && <footer className="g-statusbar">{statusbar}</footer>}
    </div>
  );
}

export interface BreadcrumbsProps {
  /** Path segments; the last is the current view. */
  items: readonly string[];
}

/** Title-bar path: workspace / section / view. */
export function Breadcrumbs({ items }: BreadcrumbsProps): ReactElement {
  return (
    <ol className="g-crumbs">
      {items.map((item, i) => (
        <li key={`${item}-${i}`} {...(i === items.length - 1 ? { 'aria-current': 'page' as const } : {})}>{item}</li>
      ))}
    </ol>
  );
}

export interface NavGroupProps {
  label: string;
  children: ReactNode;
}

/** A labelled group of sidebar items. */
export function NavGroup({ label, children }: NavGroupProps): ReactElement {
  return (
    <div role="group" aria-label={label} className="g-nav-group">
      <div className="g-menu-group" aria-hidden="true">{label}</div>
      {children}
    </div>
  );
}

export interface NavItemProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'children'> {
  icon: IconName;
  label: string;
  count?: number;
  /** The current view. */
  current?: boolean;
}

/** Sidebar link. Renders an anchor; pass `href`, or `onClick` for client routing. */
export function NavItem({ icon, label, count, current = false, className, ...rest }: NavItemProps): ReactElement {
  return (
    <a className={cx('g-nav-item', className)} title={label} {...(current ? { 'aria-current': 'page' as const } : {})} {...rest}>
      <Icon name={icon} />
      <span className="g-nav-label">{label}</span>
      {count !== undefined && <span className="g-count">{count}</span>}
    </a>
  );
}
