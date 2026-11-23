# ADR 0019: Graphite design system instead of MUI

**Status:** proposed · **Date:** 2026-11-23

## Context

[ADR 0011](0011-ui-shell.md) chose MUI as the component library. The web UI
is a dense, data-heavy desktop tool: tables, filters, charts, dialogs. MUI's
Material look is roomy and generic, and its styling (Emotion, `sx`) spreads
design decisions over every component.

**Graphite** is a design system made for exactly this kind of app: dark
first with a light theme from the same tokens, 13px base text, 28px rows and
controls, a categorical chart palette tuned for color-vision deficiencies.
It comes as a small React library (`@graphite/react`: app shell, buttons,
fields, select, checkbox/switch/radio, tabs, segmented control, badge,
panel, data table, menu, tooltip, filter bar, simple SVG charts) styled with
plain CSS over design tokens. Its source of truth is the design system
project; the library is not published to a registry.

What Graphite does not have, but the app needs: modal dialogs, alerts,
editable comboboxes (pick one, free text with suggestions, several values
as chips), progress and skeletons, removable chips, accordions, a text area,
and an icon set larger than its 28 bundled icons.

## Decision

- **Graphite replaces MUI entirely.** `@mui/material`, `@mui/icons-material`
  and Emotion are removed.
- **Vendored source:** the library is copied into `frontend/graphite/`
  (`src/`, `styles/`) and imported as `@graphite/react` through a path
  alias (`tsconfig.app.json`, `vite.config.ts`), the name it has upstream.
  No build step, no tarball, and the Docker build context (`frontend/`)
  contains it. Local changes are kept small and listed in
  `frontend/graphite/README.md`, so a new version from the design system can
  be copied over and the changes applied again.
- **What the app adds** lives in `src/ui/`, built on Graphite's tokens and,
  where one fits, its CSS classes (`g-input`, `g-menu`, `g-btn`,
  `g-table`): `Dialog`, `Alert`, `Select`/`MultiSelect`, `Combobox`,
  `SuggestField`, `TagField`, `Tooltip`, `Button`/`IconButton` with Lucide
  icons, `Chip`, `Disclosure`, `TextArea`, `ProgressBar`, `Spinner`,
  `Skeleton`, `Table`, `Stack` and a `useMediaQuery` hook. Popups (lists,
  tooltips) are rendered into `<body>` with fixed positions, so tables and
  dialogs do not clip them; Graphite's own `Select`, `Menu` and `Tooltip`
  render inline and are used only where nothing clips them (the color mode
  menu in the title bar). Components that prove themselves are candidates
  for the design system.
- **Icons: Lucide** (`lucide-react`), which Graphite names as its full icon
  set: the same outline style at 16px with 1.5px strokes (set once with
  `LucideProvider`). Icons are imported by name and tree-shaken.
- **Fonts:** Geist and Geist Mono from `@fontsource`, bundled with the app.
  The upstream style sheet loads them from Google Fonts; that import is
  removed, so the UI makes no third-party requests.
- **Styling:** Graphite's type classes and tokens, plus plain CSS files per
  area (`src/runs/runs.css`, classes prefixed `tr-<area>-`) and the
  `ui-` classes of `src/ui/ui.css`. No CSS-in-JS, no CSS modules.
- **Shell:** composed from Graphite's shell classes (`g-shell`,
  `g-titlebar`, `g-sidebar`, `g-nav-item`) rather than its `AppShell`
  component, which always renders the sidebar: on narrow screens the menu
  must leave the page and come back as an overlay. On wide screens it
  collapses to Graphite's icon rail instead of disappearing.
- **Color mode:** system, light or dark, as before, in the title bar.
  Graphite's `ThemeProvider` sets `data-theme` on `<html>`; the choice is
  kept in `localStorage` (`track-record.color-mode`), and an inline script
  in `index.html` applies it before the app loads.
- **Charts** stay ECharts ([ADR 0015](0015-charting-library.md)).
  `useChartColors()` now reads Graphite's tokens (`--chart-1…8`,
  `--chart-grid`, `--chart-axis`, text colors) for the resolved theme.
  Graphite's SVG `LineChart` cannot draw thousands of points per line with
  their own x values, which the series charts need.
- **Expression editor** stays CodeMirror ([ADR 0016](0016-expression-editor.md));
  its theme uses Graphite's CSS variables instead of MUI's.

## Consequences

- The UI is denser and dark by default when the system is dark; every
  page was restyled. Tests keep querying by role and name, so they barely
  changed: Graphite and `src/ui` keep the roles MUI had (combobox, listbox,
  option, dialog, switch, tab, radio, tooltip).
- The bundle loses MUI and Emotion (the old `mui` chunk); `lucide-react`
  is its own `icons` chunk.
- More components are ours to maintain (`src/ui`), notably the comboboxes
  and the dialog's focus handling. They are covered by `src/ui/ui.test.tsx`
  and by the page tests.
- Updating Graphite means copying `src/` and `styles/` from the design
  system again and re-applying the changes listed in its README; the type
  check shows what broke.
