import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { navItems } from './navigation'
import { renderApp as renderAt } from './test/render'
import { setViewportWidth } from './test/viewport'

const sideMenu = () =>
  screen.getByRole('navigation', { name: 'Main navigation' })

describe('App shell', () => {
  it('shows the app bar with the app name', () => {
    renderAt('/runs')
    const banner = screen.getByRole('banner')
    expect(
      within(banner).getByRole('link', { name: 'Track Record home' }),
    ).toHaveAttribute('href', '/runs')
  })

  it('redirects / to the run list', async () => {
    const router = renderAt('/')
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Runs' }),
    ).toBeInTheDocument()
    expect(router.state.location.pathname).toBe('/runs')
  })

  it('lists every view in the side menu, grouped in sections', () => {
    renderAt('/runs')
    const menu = sideMenu()
    for (const section of ['Analysis', 'Definitions', 'Administration']) {
      expect(within(menu).getByRole('list', { name: section })).toBeVisible()
    }
    const links = within(menu).getAllByRole('link')
    expect(links.map((link) => link.getAttribute('href'))).toEqual(
      navItems.map((item) => item.path),
    )
  })

  it('navigates through the side menu and marks the current view', async () => {
    const user = userEvent.setup()
    const router = renderAt('/runs')
    await user.click(within(sideMenu()).getByRole('link', { name: 'Trends' }))
    expect(router.state.location.pathname).toBe('/trends')
    expect(
      screen.getByRole('heading', { level: 1, name: 'Trends' }),
    ).toBeInTheDocument()
    expect(
      within(sideMenu()).getByRole('link', { name: 'Trends' }),
    ).toHaveAttribute('aria-current', 'page')
    expect(document.title).toBe('Trends · Track Record')
  })

  it('keeps Runs selected on a run detail page', () => {
    renderAt('/runs/3f1c')
    expect(
      screen.getByRole('heading', { level: 1, name: 'Run' }),
    ).toBeInTheDocument()
    expect(
      within(sideMenu()).getByRole('link', { name: 'Runs' }),
    ).toHaveClass('Mui-selected')
    expect(screen.getByRole('link', { name: 'All runs' })).toHaveAttribute(
      'href',
      '/runs',
    )
  })

  it('shows a not found page for unknown paths', () => {
    renderAt('/nope')
    expect(
      screen.getByRole('heading', { level: 1, name: 'Page not found' }),
    ).toBeInTheDocument()
    expect(screen.getByText('There is no page at /nope.')).toBeInTheDocument()
  })

  it('hides and shows the docked menu on wide screens', async () => {
    const user = userEvent.setup()
    renderAt('/runs')
    const toggle = screen.getByRole('button', { name: 'Close navigation' })
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await user.click(toggle)
    const reopen = screen.getByRole('button', { name: 'Open navigation' })
    expect(reopen).toHaveAttribute('aria-expanded', 'false')
    // A hidden menu is out of the accessibility tree.
    await waitFor(() =>
      expect(
        screen.queryByRole('navigation', { name: 'Main navigation' }),
      ).not.toBeInTheDocument(),
    )
    await user.click(reopen)
    expect(await screen.findByRole('navigation')).toBeVisible()
  })

  it('opens the menu over the page on narrow screens and closes it after navigating', async () => {
    setViewportWidth(600)
    const user = userEvent.setup()
    const router = renderAt('/runs')
    expect(
      screen.queryByRole('navigation', { name: 'Main navigation' }),
    ).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Open navigation' }))
    await user.click(within(sideMenu()).getByRole('link', { name: 'Aliases' }))

    expect(router.state.location.pathname).toBe('/aliases')
    await waitFor(() =>
      expect(
        screen.queryByRole('navigation', { name: 'Main navigation' }),
      ).not.toBeInTheDocument(),
    )
  })

  it('switches the color mode', async () => {
    const user = userEvent.setup()
    renderAt('/runs')
    await user.click(screen.getByRole('button', { name: 'Color mode' }))
    await user.click(screen.getByRole('menuitem', { name: 'Dark' }))
    await waitFor(() =>
      expect(document.documentElement).toHaveAttribute(
        'data-mui-color-scheme',
        'dark',
      ),
    )
  })
})
