import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// The dev server proxies the UI API to a locally running backend (UI port,
// TR_UI_ADDR, default 8081). Override with TR_UI_API_URL.
const uiApiTarget = process.env.TR_UI_API_URL ?? 'http://localhost:8081'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/ui/api': { target: uiApiTarget, changeOrigin: true },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    css: false,
  },
})
