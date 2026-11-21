// Runs before every test file (vite.config.ts → test.setupFiles).
import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, beforeEach } from 'vitest'
import '../i18n'
import { setViewportWidth } from './viewport'

// jsdom has no matchMedia; MUI's useMediaQuery needs it. Tests run on a wide
// screen unless they call setViewportWidth().
beforeEach(() => {
  setViewportWidth(1280)
})

// Testing Library only cleans up automatically with Vitest globals, which are
// off; unmount rendered trees after each test ourselves.
afterEach(() => {
  cleanup()
  localStorage.clear()
  document.documentElement.removeAttribute('data-mui-color-scheme')
})
