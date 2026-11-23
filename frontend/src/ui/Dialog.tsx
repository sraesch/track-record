import { cx } from '@graphite/react'
import { X } from 'lucide-react'
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
} from 'react'
import { useTranslation } from 'react-i18next'
import { IconButton } from './Button'
import { Portal } from './Popover'

type Props = {
  open: boolean
  title: ReactNode
  /** Escape, the backdrop and the close button call it; leave it out while closing is not allowed. */
  onClose?: () => void
  /** Called after the dialog has closed, e.g. to reset its inputs. */
  onExited?: () => void
  /** The dialog is a form: Enter in a text field submits it. */
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void
  /** Buttons at the bottom, the main action last. */
  actions?: ReactNode
  /** Maximum width: sm 480px (default), md 640px, lg 960px, xl 1280px, full the whole screen. */
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
  /** A close button in the title bar. */
  closeButton?: boolean
  /** Next to the title, e.g. actions on what the dialog shows. */
  titleActions?: ReactNode
  /** Accessible name when `title` is not plain text. */
  'aria-label'?: string
  /** The body without padding and gaps, e.g. for a preview that fills it. */
  flush?: boolean
  children?: ReactNode
}

const focusable =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]), [contenteditable="true"]'

/**
 * A modal dialog (WAI-ARIA dialog pattern): rendered into <body>, the rest
 * of the page hidden from assistive technology and inert, the focus kept
 * inside. It moves to the first text field (or the first control) when the
 * dialog opens and back to where it was when it closes.
 */
export function Dialog({ open, onExited, ...props }: Props) {
  const wasOpen = useRef(open)
  useEffect(() => {
    if (wasOpen.current && !open) onExited?.()
    wasOpen.current = open
  }, [open, onExited])
  return open ? <OpenDialog {...props} /> : null
}

function OpenDialog({
  title,
  onClose,
  onSubmit,
  actions,
  size = 'sm',
  closeButton = false,
  titleActions,
  flush = false,
  children,
  ...aria
}: Omit<Props, 'open' | 'onExited'>) {
  const { t } = useTranslation()
  const titleId = useId()
  const dialog = useRef<HTMLElement | null>(null)
  const backdrop = useRef<HTMLDivElement>(null)
  const closeRef = useRef(onClose)
  useLayoutEffect(() => {
    closeRef.current = onClose
  })

  // Hide the rest of the page while the dialog is open.
  useLayoutEffect(() => {
    const own = backdrop.current
    const hidden: Element[] = []
    for (const element of Array.from(document.body.children)) {
      if (element === own || element.contains(own)) continue
      if (element.getAttribute('aria-hidden') === 'true' || element.hasAttribute('inert')) continue
      // Popovers and tooltips opened from the dialog come later.
      element.setAttribute('aria-hidden', 'true')
      element.setAttribute('inert', '')
      hidden.push(element)
    }
    return () => {
      for (const element of hidden) {
        element.removeAttribute('aria-hidden')
        element.removeAttribute('inert')
      }
    }
  }, [])

  // Focus in on open, back out on close.
  useLayoutEffect(() => {
    const before = document.activeElement
    const node = dialog.current
    if (node) {
      const first =
        node.querySelector<HTMLElement>('.ui-dialog-body input:not([disabled]), .ui-dialog-body textarea:not([disabled])') ??
        node.querySelector<HTMLElement>(`.ui-dialog-body :is(${focusable})`) ??
        node.querySelector<HTMLElement>(`.ui-dialog-actions :is(${focusable})`)
      ;(first ?? node).focus()
    }
    return () => {
      if (before instanceof HTMLElement && before.isConnected) before.focus()
    }
  }, [])

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape' && !event.defaultPrevented) {
      event.stopPropagation()
      closeRef.current?.()
      return
    }
    if (event.key !== 'Tab' || !dialog.current) return
    const items = Array.from(dialog.current.querySelectorAll<HTMLElement>(focusable))
    const first = items[0]
    const last = items[items.length - 1]
    if (!first || !last) {
      event.preventDefault()
      return
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  const shared = {
    ref: (node: HTMLElement | null) => {
      dialog.current = node
    },
    role: 'dialog',
    'aria-modal': true,
    ...(aria['aria-label'] ? { 'aria-label': aria['aria-label'] } : { 'aria-labelledby': titleId }),
    tabIndex: -1,
    className: cx('ui-dialog', `ui-dialog-${size}`),
    onKeyDown,
  } as const
  const content = (
    <>
      <div className="ui-dialog-head">
        <h2 id={titleId} className="ui-dialog-title">
          {title}
        </h2>
        {titleActions}
        {closeButton && onClose && <IconButton icon={X} label={t('common.close')} onClick={onClose} />}
      </div>
      <div className="ui-dialog-body" style={flush ? { padding: 0, gap: 0 } : undefined}>
        {children}
      </div>
      {actions && <div className="ui-dialog-actions">{actions}</div>}
    </>
  )
  return (
    <Portal>
      <div
        ref={backdrop}
        className="ui-backdrop"
        onPointerDown={(event) => {
          if (event.target === event.currentTarget) closeRef.current?.()
        }}
      >
        {onSubmit ? (
          <form {...shared} noValidate onSubmit={onSubmit}>
            {content}
          </form>
        ) : (
          <div {...shared}>{content}</div>
        )}
      </div>
    </Portal>
  )
}
