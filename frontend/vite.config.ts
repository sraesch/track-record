import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// The dev server proxies the UI API to a locally running backend (UI port,
// TR_UI_ADDR, default 8081). Override with TR_UI_API_URL.
const uiApiTarget = process.env.TR_UI_API_URL ?? 'http://localhost:8081'

// react-markdown and the unified/micromark packages it is built on.
const markdownPackages =
  /node_modules[\\/](react-markdown|remark-[^\\/]+|rehype-[^\\/]+|unified|micromark[^\\/]*|mdast-util-[^\\/]+|hast-util-[^\\/]+|unist-util-[^\\/]+|vfile[^\\/]*|property-information|character-entities[^\\/]*|parse-entities|stringify-entities|decode-named-character-reference|style-to-object|inline-style-parser|markdown-table)[\\/]/

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Outside node_modules, so `npm ci` (test.sh) does not delete the
  // pre-bundled dependencies under a running dev server; it would then
  // fail every module it has not served yet with "Outdated Optimize Dep".
  cacheDir: '.vite',
  build: {
    rolldownOptions: {
      output: {
        // Libraries change rarely; keep them apart from the app code so
        // browsers keep them cached across releases.
        codeSplitting: {
          groups: [
            { name: 'mui', test: /node_modules[\\/](@mui|@emotion)[\\/]/ },
            // The Markdown renderer is loaded only when a Markdown
            // attachment is opened; in `vendor` it would load with the app.
            // Small helpers it shares with others stay in `vendor`.
            { name: 'markdown', test: markdownPackages },
            { name: 'vendor', test: /node_modules[\\/]/ },
          ],
        },
      },
    },
  },
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
