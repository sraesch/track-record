// Runs before every test file (vite.config.ts → test.setupFiles).
import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, beforeEach, vi } from 'vitest'
import '../i18n'
import { mockApi } from './api'
import { setViewportWidth } from './viewport'

// jsdom has no matchMedia; MUI's useMediaQuery needs it. Tests run on a wide
// screen unless they call setViewportWidth().
beforeEach(() => {
  setViewportWidth(1280)
  // No test talks to a real server; tests call mockApi() with their data.
  mockApi()
})

// Testing Library only cleans up automatically with Vitest globals, which are
// off; unmount rendered trees after each test ourselves.
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  localStorage.clear()
  document.documentElement.removeAttribute('data-mui-color-scheme')
})
