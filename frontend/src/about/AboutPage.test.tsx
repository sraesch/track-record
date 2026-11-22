import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { renderApp } from '../test/render'

describe('About page', () => {
  it('opens from the app bar', async () => {
    const user = userEvent.setup()
    const router = renderApp('/runs')
    await user.click(
      within(screen.getByRole('banner')).getByRole('link', { name: 'About' }),
    )
    expect(router.state.location.pathname).toBe('/about')
    expect(
      screen.getByRole('heading', { level: 1, name: 'About Track Record' }),
    ).toBeInTheDocument()
  })

  it('shows the app changelog formatted as Markdown', async () => {
    renderApp('/about')
    expect(document.title).toBe('About Track Record · Track Record')
    // Headings and lists of APP_CHANGELOG.md, not its Markdown source.
    expect(
      await screen.findByRole('heading', { level: 2, name: 'Unreleased' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { level: 3, name: 'Runs' }),
    ).toBeInTheDocument()
    expect(screen.getAllByRole('listitem').length).toBeGreaterThan(5)
    expect(screen.queryByText(/^## /)).not.toBeInTheDocument()
  })
})
