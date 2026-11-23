# Graphite (vendored)

The Graphite design system's React library (`@graphite/react` 0.1.0), copied
here from the design system project and imported as `@graphite/react`
through an alias (`tsconfig.app.json`, `vite.config.ts`).
[ADR 0019](../../docs/adr/0019-graphite-design-system.md) explains why.
`UPSTREAM-README.md` is the library's own README.

- `src/` — components and hooks (TypeScript, React)
- `styles/tokens.css` — design tokens (colors per theme, spacing, type)
- `styles/graphite.css` — component styles (classes prefixed `g-`)

## Updating

Copy `src/` and `styles/` from a new version over these, then apply the
local changes below again (or drop those upstream has taken over) and run
`npm run typecheck` and the tests.

## Local changes

1. **React 19 types** (`src/utils/hooks.ts`): `useDismiss` takes
   `RefObject<HTMLElement | null>[]` and `useElementWidth` returns
   `RefObject<T | null>`, because `useRef<T>(null)` has that type in React 19.
2. **No Google Fonts** (`styles/graphite.css`): the `@import` of Geist from
   fonts.googleapis.com is removed; the app bundles Geist and Geist Mono
   from `@fontsource` (`src/main.tsx`).
3. **Select is a combobox** (`src/components/Select.tsx`): the trigger has
   `role="combobox"` and is named by its label only (WAI-ARIA select-only
   combobox; the value is its content).
4. **Translatable names** (`src/components/AppShell.tsx`,
   `src/components/FilterBar.tsx`): `AppShell` takes `sidebarLabel`,
   `FilterBar` takes `label` and `FilterChip` takes `removeLabel`; the
   English texts stay the defaults.
