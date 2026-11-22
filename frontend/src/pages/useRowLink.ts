import type { MouseEvent } from 'react'
import { useNavigate } from 'react-router'

/**
 * Lets a whole table row open a page, in addition to the link the row must
 * also contain (for keyboards, screen readers and opening in a new tab).
 *
 *   const rowLink = useRowLink()
 *   <TableRow hover onClick={rowLink(href)} sx={{ cursor: 'pointer' }}>
 */
export function useRowLink() {
  const navigate = useNavigate()
  return (href: string) => (event: MouseEvent<HTMLElement>) => {
    // Let links and buttons inside the row do their own thing.
    const target = event.target as HTMLElement
    if (target.closest('a, button, [role="button"]')) return
    // Modified clicks (new tab, …) are left to the row's link.
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    // Selecting text (e.g. to copy a commit) is not a click.
    if (window.getSelection()?.toString()) return
    navigate(href)
  }
}
