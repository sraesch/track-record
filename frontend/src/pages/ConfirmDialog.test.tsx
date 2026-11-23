import { TextField } from '@graphite/react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { ConfirmDialog } from './ConfirmDialog'

function renderDialog() {
  render(
    <ConfirmDialog
      open
      title="Rename"
      confirmLabel="Save"
      onConfirm={() => {}}
      onClose={() => {}}
    >
      <TextField label="First" />
      <TextField label="Second" />
    </ConfirmDialog>,
  )
  return {
    first: screen.getByRole('textbox', { name: 'First' }),
    second: screen.getByRole('textbox', { name: 'Second' }),
  }
}

/** Lets effects and timers that could move the focus run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 400))

describe('ConfirmDialog', () => {
  it('focuses the first field once open', async () => {
    const { first } = renderDialog()
    await waitFor(() => expect(first).toHaveFocus())
  })

  it('keeps a focus the user moved right after opening', async () => {
    const user = userEvent.setup()
    const { first, second } = renderDialog()
    await user.click(second)
    await settle()
    expect(second).toHaveFocus()
    expect(first).not.toHaveFocus()
  })
})
