import {
  cloneElement,
  type ReactElement,
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from 'react'
import { Portal } from './Popover'
import { usePopoverStyle } from './usePopover'

type ChildProps = {
  'aria-label'?: string
  'aria-describedby'?: string
}

type Props = {
  /** Short, supplementary text. Nothing shows when it is empty. */
  title: ReactNode
  /** One element; hovering or focusing it shows the tooltip. */
  children: ReactElement<ChildProps>
  /**
   * The title also names the child (`aria-label`), e.g. for an icon or a
   * chip that says no more than the tooltip. Otherwise the tooltip
   * describes the child while it shows (`aria-describedby`).
   */
  labels?: boolean
  /**
   * The title also describes the child while the tooltip is closed (e.g. a
   * status whose reason is in the tooltip). The description is rendered
   * into <body>, so it does not add to the text around the child.
   */
  describes?: boolean
  /** Delay before showing, in ms. */
  delay?: number
}

/**
 * Shows `title` next to its child on hover and keyboard focus; Escape
 * hides it. Rendered into <body>, so tables and dialogs do not clip it.
 */
export function Tooltip({
  title,
  children,
  labels = false,
  describes = false,
  delay = 300,
}: Props) {
  const [open, setOpen] = useState(false)
  const anchor = useRef<HTMLSpanElement>(null)
  const timer = useRef<number | undefined>(undefined)
  const id = useId()
  const descriptionId = `${id}-description`
  const popup = useRef<HTMLSpanElement>(null)
  const style = usePopoverStyle(anchor, open, { placement: 'top', popup })
  useEffect(() => () => window.clearTimeout(timer.current), [])

  const empty = title === undefined || title === null || title === ''
  const show = () => {
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setOpen(true), delay)
  }
  const hide = () => {
    window.clearTimeout(timer.current)
    setOpen(false)
  }

  const child = cloneElement(children, {
    ...(labels && typeof title === 'string' && !children.props['aria-label']
      ? { 'aria-label': title }
      : {}),
    ...(describes
      ? {
          'aria-describedby': [
            children.props['aria-describedby'],
            descriptionId,
          ]
            .filter(Boolean)
            .join(' '),
        }
      : open && !labels
        ? {
            'aria-describedby': [children.props['aria-describedby'], id]
              .filter(Boolean)
              .join(' '),
          }
        : {}),
  })

  if (empty) return children
  return (
    // Hover and focus of the child show the tooltip; the child is what
    // users interact with, the wrapper only listens.
    // oxlint-disable-next-line jsx-a11y/no-static-element-interactions
    <span
      ref={anchor}
      className="ui-tip-anchor"
      onPointerEnter={show}
      onPointerLeave={hide}
      onFocus={show}
      onBlur={hide}
      onKeyDown={(event) => {
        if (event.key === 'Escape') hide()
      }}
    >
      {child}
      {describes && (
        <Portal>
          <span id={descriptionId} hidden>
            {title}
          </span>
        </Portal>
      )}
      {open && (
        <Portal>
          <span
            ref={popup}
            id={id}
            role="tooltip"
            className="g-tooltip ui-tooltip"
            style={style}
          >
            {title}
          </span>
        </Portal>
      )}
    </span>
  )
}
