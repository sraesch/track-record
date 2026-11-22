import TextField from '@mui/material/TextField'
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

/** Longer than the dialog's fade-in, so it has ended afterwards. */
const fadeIn = () => new Promise((resolve) => setTimeout(resolve, 400))

describe('ConfirmDialog', () => {
  it('focuses the first field once open', async () => {
    const { first } = renderDialog()
    await waitFor(() => expect(first).toHaveFocus())
  })

  it('keeps a focus the user moved while the dialog faded in', async () => {
    const user = userEvent.setup()
    const { first, second } = renderDialog()
    await user.click(second)
    await fadeIn()
    expect(second).toHaveFocus()
    expect(first).not.toHaveFocus()
  })
})
