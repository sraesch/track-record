// Runs before every test file (vite.config.ts → test.setupFiles).
import '@testing-library/jest-dom/vitest'
import { configure } from '@testing-library/react'
import { afterEach, beforeEach, vi } from 'vitest'
import '../i18n'
import { mockApi } from './api'
import { resetDocument } from './reset'
import { setViewportWidth } from './viewport'

// findBy* and waitFor give up after 3 s instead of 1 s. They wait for a
// page to render its data, 0.1-0.4 s on an idle machine; under load spikes
// that took over 1 s (testTimeout in vite.config.ts says more).
configure({ asyncUtilTimeout: 3000 })

// jsdom has no canvas to draw charts on; tests see a list of their lines.
vi.mock('../charts/EChart', () => import('./fakeChart'))

// jsdom does not lay out text, which CodeMirror needs; tests type into a
// text area instead.
vi.mock('../dsl/DslEditor', () => import('./fakeDslEditor'))

// jsdom has no matchMedia; MUI's useMediaQuery needs it. Tests run on a wide
// screen unless they call setViewportWidth().
beforeEach(() => {
  setViewportWidth(1280)
  // No test talks to a real server; tests call mockApi() with their data.
  mockApi()
})

afterEach(resetDocument)
