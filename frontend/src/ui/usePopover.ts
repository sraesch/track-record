import {
  type CSSProperties,
  type RefObject,
  useEffect,
  useLayoutEffect,
  useState,
} from 'react'

export type Placement = 'bottom-start' | 'bottom-end' | 'top'

const gap = 4
const margin = 8

/**
 * Fixed position for a popup next to `anchor` while `open`, kept in place
 * when the page scrolls or resizes. Below the anchor, or above it when there
 * is more room there; `maxHeight` is the room left.
 */
export function usePopoverStyle(
  anchor: RefObject<HTMLElement | null>,
  open: boolean,
  {
    placement = 'bottom-start',
    matchWidth = false,
    popup,
  }: {
    placement?: Placement
    matchWidth?: boolean
    /** The popup itself, to keep a centered one (`top`) inside the window. */
    popup?: RefObject<HTMLElement | null>
  } = {},
): CSSProperties {
  const [style, setStyle] = useState<CSSProperties>({})

  useLayoutEffect(() => {
    if (!open) return
    const update = () => {
      const element = anchor.current
      if (!element) return
      const rect = element.getBoundingClientRect()
      const below = window.innerHeight - rect.bottom - gap - margin
      const above = rect.top - gap - margin
      const up =
        placement === 'top'
          ? above > 40 || above > below
          : below < 200 && above > below
      const next: CSSProperties = {
        maxHeight: Math.max(120, up ? above : below),
        ...(matchWidth ? { minWidth: rect.width } : {}),
      }
      if (up) next.bottom = window.innerHeight - rect.top + gap
      else next.top = rect.bottom + gap
      if (placement === 'bottom-end')
        next.right = window.innerWidth - rect.right
      else if (placement === 'top') {
        const width = popup?.current?.offsetWidth ?? 0
        const center = rect.left + rect.width / 2
        next.left = Math.max(
          margin,
          Math.min(center - width / 2, window.innerWidth - margin - width),
        )
      } else next.left = Math.max(margin, rect.left)
      setStyle(next)
    }
    update()
    window.addEventListener('scroll', update, true)
    window.addEventListener('resize', update)
    return () => {
      window.removeEventListener('scroll', update, true)
      window.removeEventListener('resize', update)
    }
  }, [anchor, open, placement, matchWidth, popup])

  return style
}

/**
 * Calls `onDismiss` on a pointer press outside all `elements` while
 * `active` (e.g. to close a list when the user clicks elsewhere).
 */
export function useDismiss(
  elements: readonly RefObject<HTMLElement | null>[],
  active: boolean,
  onDismiss: () => void,
) {
  useEffect(() => {
    if (!active) return
    const onDown = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (elements.some((ref) => ref.current?.contains(target))) return
      onDismiss()
    }
    document.addEventListener('pointerdown', onDown)
    return () => document.removeEventListener('pointerdown', onDown)
    // Refs are stable objects; callers pass a new array every render.
  }, [active, onDismiss]) // oxlint-disable-line react/exhaustive-deps
}
