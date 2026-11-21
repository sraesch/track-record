# ADR 0011: UI shell, component library and routing

**Status:** proposed · **Date:** 2026-11-21

## Context

[ADR 0010](0010-frontend-scaffold.md) left the component library, layout and
routing open. The web UI has a fixed set of views (requirements, section 4):
run list and detail, comparison (FR-16), trends (FR-18), series overlay
(FR-19), definitions, derived metrics, aliases and API tokens. Every run,
case execution and analysis view needs a URL that reproduces it, filters
included (FR-21). The UI is data-heavy: tables, filters, dialogs, forms.

## Decision

- **Component library: MUI** (`@mui/material`, Emotion, `@mui/icons-material`).
  Mature, accessible components for exactly this kind of app (app bar,
  drawer, lists, tables, dialogs, forms), a theme with light and dark color
  schemes, and room to grow (MUI X data grid and charts) without a second
  design system. Icons are imported per file
  (`@mui/icons-material/ShowChart`) to keep dev builds fast.
- **Routing: React Router** in data mode (`createBrowserRouter`). Routes are
  in `src/routes.tsx`; tests render them with `createMemoryRouter`. View
  state goes into the URL query string, not component state (FR-21).
- **Shell:** an app bar (menu toggle, app name linking home, color mode
  menu) and a side menu with three sections: *Analysis* (runs, compare,
  trends, series), *Definitions* (suites & scenarios, metrics, derived
  metrics, aliases) and *Administration* (API tokens). On wide screens
  (`md` and up) the menu is docked and can be hidden; on narrow screens it
  slides in over the page and closes after navigating.
- **One list for menu and routes:** `src/navigation.ts` lists the views;
  the menu and the routes are built from it. `/` redirects to the run list;
  sub-pages (`/runs/:runId`) keep their view selected in the menu.
- **Color mode:** system, light or dark, chosen in the app bar. MUI keeps
  the choice in `localStorage` and sets `data-mui-color-scheme` on `<html>`.
  This is a per-browser preference, not view state, so it is not in the URL.
- **Page titles:** `PageHeader` renders the page heading and sets the
  browser tab title (`Trends · Track Record`).
- **Bundles:** the build puts MUI and the other libraries into their own
  chunks, so they stay cached when app code changes.

## Consequences

- Views not built yet show a placeholder with their description; each view
  replaces its placeholder when it is built.
- Tests run on a wide screen by default; `setViewportWidth()` emulates other
  widths for `useMediaQuery` (jsdom has no `matchMedia`).
- MUI's own texts (e.g. table pagination) will need its locale objects once
  there is a second UI language.

## Left for later ADRs

API client generated from `backend/openapi/ui.json` · data fetching and
caching · charting library (MUI X charts is a candidate) · URL schema for
view state.
