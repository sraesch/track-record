import { cleanup } from '@testing-library/react'
import { vi } from 'vitest'

/**
 * Leaves the document as a test file starts it: rendered trees unmounted,
 * stubs and the stored color mode gone.
 */
export function resetDocument() {
  // Testing Library only cleans up automatically with Vitest globals,
  // which are off.
  cleanup()
  vi.unstubAllGlobals()
  localStorage.clear()
  document.documentElement.removeAttribute('data-theme')
  document.documentElement.style.colorScheme = ''
}
