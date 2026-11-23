import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// The Graphite design system is vendored in graphite/ (ADR 0019) and
// imported as `@graphite/react`, like the package it is built as upstream.
const graphite = (path: string) =>
  fileURLToPath(new URL(`./graphite/${path}`, import.meta.url))

// The dev server proxies the UI API to a locally running backend (UI port,
// TR_UI_ADDR, default 8081). Override with TR_UI_API_URL.
const uiApiTarget = process.env.TR_UI_API_URL ?? 'http://localhost:8081'

// react-markdown and the unified/micromark packages it is built on.
const markdownPackages =
  /node_modules[\\/](react-markdown|remark-[^\\/]+|rehype-[^\\/]+|unified|micromark[^\\/]*|mdast-util-[^\\/]+|hast-util-[^\\/]+|unist-util-[^\\/]+|vfile[^\\/]*|property-information|character-entities[^\\/]*|parse-entities|stringify-entities|decode-named-character-reference|style-to-object|inline-style-parser|markdown-table)[\\/]/

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      { find: /^@graphite\/react$/, replacement: graphite('src/index.ts') },
      {
        find: /^@graphite\/react\/styles\.css$/,
        replacement: graphite('styles/graphite.css'),
      },
      {
        find: /^@graphite\/react\/tokens\.css$/,
        replacement: graphite('styles/tokens.css'),
      },
    ],
  },
  // Outside node_modules, so `npm ci` (test.sh) does not delete the
  // pre-bundled dependencies under a running dev server; it would then
  // fail every module it has not served yet with "Outdated Optimize Dep".
  cacheDir: '.vite',
  build: {
    // Fonts stay files: inlined into the CSS, every subset (Cyrillic,
    // Vietnamese, ...) would load, not only those a page's text needs.
    assetsInlineLimit: (file) => (/\.woff2?$/.test(file) ? false : undefined),
    rolldownOptions: {
      output: {
        // Libraries change rarely; keep them apart from the app code so
        // browsers keep them cached across releases.
        codeSplitting: {
          groups: [
            { name: 'icons', test: /node_modules[\\/]lucide-react[\\/]/ },
            // The Markdown renderer is loaded only when a Markdown
            // attachment is opened; in `vendor` it would load with the app.
            // Small helpers it shares with others stay in `vendor`.
            { name: 'markdown', test: markdownPackages },
            // ECharts is loaded with the first chart (ADR 0015).
            { name: 'charts', test: /node_modules[\\/](echarts|zrender)[\\/]/ },
            // CodeMirror is loaded with the first expression editor
            // (ADR 0016).
            {
              name: 'editor',
              test: /node_modules[\\/](@codemirror|@lezer|style-mod|crelt|w3c-keyname)[\\/]/,
            },
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
    // Every worker renders the whole app with React's development build
    // in jsdom, which keeps a core busy. With Vitest's default (all cores
    // but one) the workers slow each other down: on 10 cores the slowest
    // test took 2.4 s with 9 workers and 1.5 s with 4, while the whole
    // run took as long. Half the cores leaves room for a loaded machine
    // or CI runner, where tests otherwise ran into the 5 s timeout.
    maxWorkers: '50%',
    // Three times Vitest's default. The heaviest tests render the whole
    // app and need about 1 s of CPU (1.3 s in a full run on an idle
    // machine, measured with MUI), nearly all of it rendering, not the tests.
    // During load spikes (load average 45-99 on 10 cores) tests ran up to
    // 5-10 times slower than idle, and even 0.5 s tests hit 5 s. Keep tests
    // small instead of raising this further; see enterText().
    testTimeout: 15_000,
  },
})
