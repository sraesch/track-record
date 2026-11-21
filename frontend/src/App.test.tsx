import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('App', () => {
  it('renders the translated app name', () => {
    render(<App />)
    expect(
      screen.getByRole('heading', { level: 1, name: 'Track Record' }),
    ).toBeInTheDocument()
  })

  it('does not render raw translation keys', () => {
    render(<App />)
    expect(screen.queryByText(/^(app|home)\./)).not.toBeInTheDocument()
  })
})
