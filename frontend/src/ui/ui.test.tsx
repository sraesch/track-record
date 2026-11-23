import { TextField } from '@graphite/react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { Combobox, Dialog, MultiSelect, Select, SuggestField, TagField, Tooltip } from '.'

describe('Select', () => {
  function Pick() {
    const [value, setValue] = useState('')
    return (
      <Select
        label="Trigger"
        value={value}
        onChange={setValue}
        options={[
          { value: '', label: 'Any' },
          { value: 'nightly', label: 'Nightly' },
          { value: 'manual', label: 'Manual' },
        ]}
      />
    )
  }

  it('is a combobox named by its label that picks an option', async () => {
    const user = userEvent.setup()
    render(<Pick />)
    const select = screen.getByRole('combobox', { name: 'Trigger' })
    expect(select).toHaveTextContent('Any')
    await user.click(select)
    await user.click(screen.getByRole('option', { name: 'Nightly' }))
    expect(select).toHaveTextContent('Nightly')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(select).toHaveFocus()
  })

  it('works with the keyboard', async () => {
    const user = userEvent.setup()
    render(<Pick />)
    screen.getByRole('combobox', { name: 'Trigger' }).focus()
    await user.keyboard('{ArrowDown}{ArrowDown}{ArrowDown}{Enter}')
    expect(screen.getByRole('combobox', { name: 'Trigger' })).toHaveTextContent('Manual')
    await user.keyboard('{ArrowDown}{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('picks several values and stays open', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn<(value: string[]) => void>()
    render(
      <MultiSelect
        label="Status"
        value={['failed']}
        onChange={onChange}
        placeholder="Any"
        options={[
          { value: 'failed', label: 'Failed' },
          { value: 'aborted', label: 'Aborted' },
        ]}
      />,
    )
    await user.click(screen.getByRole('combobox', { name: 'Status' }))
    await user.click(screen.getByRole('option', { name: 'Aborted' }))
    expect(onChange).toHaveBeenCalledWith(['failed', 'aborted'])
    expect(screen.getByRole('listbox')).toHaveAttribute('aria-multiselectable', 'true')
    expect(screen.getByRole('option', { name: 'Failed' })).toHaveAttribute('aria-selected', 'true')
  })
})

describe('Combobox', () => {
  function Pick({ onChange }: { onChange?: (v: string | null) => void }) {
    const [value, setValue] = useState<string | null>(null)
    return (
      <Combobox
        label="Scenario"
        options={['load-model', 'stream-city', 'orbit']}
        value={value}
        onChange={(v) => {
          setValue(v)
          onChange?.(v)
        }}
      />
    )
  }

  it('opens on click, filters by the typed text and shows the pick', async () => {
    const user = userEvent.setup()
    render(<Pick />)
    const input = screen.getByRole('combobox', { name: 'Scenario' })
    await user.click(input)
    expect(screen.getAllByRole('option')).toHaveLength(3)
    await user.type(input, 'city')
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual(['stream-city'])
    await user.click(screen.getByRole('option', { name: 'stream-city' }))
    expect(input).toHaveValue('stream-city')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('clears the value when the text is cleared', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn<(value: string | null) => void>()
    render(<Pick onChange={onChange} />)
    const input = screen.getByRole('combobox', { name: 'Scenario' })
    await user.click(input)
    await user.keyboard('{ArrowDown}{Enter}')
    expect(input).toHaveValue('load-model')
    await user.clear(input)
    expect(onChange).toHaveBeenLastCalledWith(null)
  })

  it('says when nothing matches', async () => {
    const user = userEvent.setup()
    render(<Pick />)
    await user.type(screen.getByRole('combobox', { name: 'Scenario' }), 'zzz')
    expect(screen.getByRole('listbox')).toHaveTextContent('No options')
  })
})

describe('SuggestField', () => {
  it('keeps free text and fills in a picked suggestion', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn<(option: string) => void>()
    function Field() {
      const [text, setText] = useState('')
      return (
        <SuggestField
          label="Unit"
          value={text}
          onChange={setText}
          onSelect={onSelect}
          options={['ms', 'Mbit/s', 'MB']}
        />
      )
    }
    render(<Field />)
    const input = screen.getByRole('combobox', { name: 'Unit' })
    await user.type(input, 'mb')
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual(['Mbit/s', 'MB'])
    await user.click(screen.getByRole('option', { name: 'MB' }))
    expect(input).toHaveValue('MB')
    expect(onSelect).toHaveBeenCalledWith('MB')
    await user.clear(input)
    await user.type(input, 'frames')
    expect(input).toHaveValue('frames')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })
})

describe('TagField', () => {
  it('adds on Enter, rejects what onAdd refuses and removes with Backspace', async () => {
    const user = userEvent.setup()
    function Field() {
      const [values, setValues] = useState<string[]>([])
      return (
        <TagField
          label="Parameters"
          values={values}
          onChange={setValues}
          onAdd={(text) => text.includes('=')}
        />
      )
    }
    render(<Field />)
    const input = screen.getByRole('textbox', { name: 'Parameters' })
    await user.type(input, 'size=10{Enter}')
    expect(screen.getByText('size=10')).toBeInTheDocument()
    expect(input).toHaveValue('')
    await user.type(input, 'fast{Enter}')
    expect(input).toHaveValue('fast')
    await user.clear(input)
    await user.keyboard('{Backspace}')
    expect(screen.queryByText('size=10')).not.toBeInTheDocument()
  })
})

describe('Dialog', () => {
  function Host() {
    const [open, setOpen] = useState(false)
    const [select, setSelect] = useState('a')
    return (
      <>
        <button type="button" onClick={() => setOpen(true)}>
          Open
        </button>
        <Dialog
          open={open}
          title="Rename"
          onClose={() => setOpen(false)}
          actions={<button type="button">Save</button>}
        >
          <TextField label="Name" />
          <Select
            label="Kind"
            value={select}
            onChange={setSelect}
            options={[
              { value: 'a', label: 'A' },
              { value: 'b', label: 'B' },
            ]}
          />
        </Dialog>
      </>
    )
  }

  it('is modal: focus inside, the page hidden, Escape closes and returns the focus', async () => {
    const user = userEvent.setup()
    render(<Host />)
    const opener = screen.getByRole('button', { name: 'Open' })
    await user.click(opener)
    const dialog = screen.getByRole('dialog', { name: 'Rename' })
    expect(within(dialog).getByRole('textbox', { name: 'Name' })).toHaveFocus()
    // The page behind is out of reach.
    expect(screen.queryByRole('button', { name: 'Open' })).not.toBeInTheDocument()

    // Escape in an open list closes the list, not the dialog.
    await user.click(within(dialog).getByRole('combobox', { name: 'Kind' }))
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(screen.getByRole('dialog')).toBeInTheDocument()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(opener).toHaveFocus()
  })

  it('keeps Tab inside', async () => {
    const user = userEvent.setup()
    render(<Host />)
    await user.click(screen.getByRole('button', { name: 'Open' }))
    await user.tab()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Save' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveFocus()
  })
})

describe('Tooltip', () => {
  it('shows on hover and can name its child', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip title="Invalid: broken device" labels>
        <button type="button">Invalid</button>
      </Tooltip>,
    )
    const chip = screen.getByLabelText('Invalid: broken device')
    await user.hover(chip)
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Invalid: broken device')
    await user.unhover(chip)
    await waitFor(() => expect(screen.queryByRole('tooltip')).not.toBeInTheDocument())
  })
})
