# Frontend

Web UI for Track Record: TypeScript (strict), React, Vite, Vitest and
react-i18next, with MUI components and React Router. Setup decisions are in
[ADR 0010](../docs/adr/0010-frontend-scaffold.md), the shell (app bar, side
menu, routing) in [ADR 0011](../docs/adr/0011-ui-shell.md).

## Adding a view

1. Add an entry to `src/navigation.ts` (id, path, icon); it appears in the
   side menu and gets a route.
2. Add `nav.items.<id>` and `pages.<id>.description` to the locale file.
3. Replace its `PlaceholderPage` route in `src/routes.tsx` with the page.
   Pages start with `<PageHeader>`; view state goes into the URL (FR-21).

## Layout

```
frontend/
  index.html            Vite entry
  src/main.tsx          Bootstraps React, i18n and the router
  src/App.tsx           Theme and router providers
  src/routes.tsx        All routes (React Router)
  src/navigation.ts     Side menu entries; routes are built from them
  src/theme.ts          MUI theme: light and dark color schemes
  src/layout/           App bar, side menu, color mode menu
  src/pages/            Shared page parts: header, sections, dialogs, search
                        field, placeholders
  src/api/              API client; schema.d.ts is generated (do not edit)
  src/runs/             Run list: filters in the URL, table, queries;
                        test case parts shared by the run views
  src/runs/detail/      Run detail: overview, tags, annotations, case
                        executions (filters in the URL), attachments,
                        invalidation and deletion
  src/runs/case/        Case execution: overview, parameters, test case
                        tags and annotations, device details,
                        measurements per sample, derived metric
                        values, a chart per series metric, attachments
  src/aliases/          Aliases (FR-14): list with filters in the URL,
                        add, change, remove and set again; typed
                        parameter values
  src/suites/           Suites & scenarios (FR-7): list and selected
                        definition with its versions and what each
                        changed; the test cases of a scenario version
                        with their tags and annotations; tab, search and
                        selection in the URL
  src/metrics/          Metrics (FR-7, FR-13a): one definition per metric
                        (no versions, ADR 0018) with edit, rename and
                        delete; unit catalogue by quantity, custom units;
                        tab and search in the URL
  src/derived/          Derived metrics (FR-17): list and selected metric
                        with uses and users, rename and delete; editor
                        that creates a metric or changes one in place (no
                        versions, ADR 0017),
                        with live validation and a preview on a case
                        execution; their values on the case execution page
  src/dsl/              The DSL editor (CodeMirror, ADR 0016, lazily
                        loaded) and its tokenizer and completion
  src/tags/             Tag editor shared by runs and test cases
  src/annotations/      Annotation list and editor shared by runs and
                        test cases
  src/charts/           Charts (ECharts, ADR 0015): lazily loaded <Chart>,
                        colors from the MUI color scheme
  src/attachments/      Attachment previews (dialog and own page): images,
                        Markdown, JSON, text
  src/format.ts         Dates, durations and commits for display
  src/i18n/             i18next setup, typed keys, locale files
    locales/en/         English translations (JSON, one file per namespace)
  src/test/setup.ts     Vitest setup: jest-dom matchers, i18n, cleanup
  src/test/viewport.ts  setViewportWidth(): screen width for useMediaQuery
  src/test/api.ts       mockApi(): fake UI API for tests (stubs fetch)
  src/test/render.tsx   renderApp(path): the whole app on a memory router
  src/test/fakeChart.tsx  Stands in for the canvas chart (jsdom has none)
  src/test/fakeDslEditor.tsx  Stands in for CodeMirror (a text area)
  tools/openapi/        Generator for src/api/schema.d.ts (own package, TypeScript 5)
  vite.config.ts        Vite and Vitest config, dev proxy to the backend
  test.sh               lint, type check, tests and build (what CI runs)
  scripts/seed.ts       Test data for UI development (npm run seed)
  Dockerfile            Image: nginx serving the build, proxying /ui/api
  nginx.conf.template   nginx config (TR_UI_API_UPSTREAM substituted at start)
```

## Commands

Needs Node.js 24 (see `.node-version`). Run in `frontend/`:

```bash
npm ci                # install from the lockfile
npm run dev           # dev server on http://localhost:5173
npm test              # Vitest, once
npm run test:watch    # Vitest in watch mode
npm run lint          # oxlint
npm run typecheck     # tsc
npm run build         # type check and production build into dist/
npm run generate:api  # regenerate src/api/schema.d.ts from backend/openapi/ui.json
```

Or everything CI checks, from the repository root:

```bash
frontend/test.sh
```

Arguments after `--` go to `vitest run`, e.g. `frontend/test.sh -- App`.

## Backend in development

The dev server proxies `/ui/api` to the backend's UI port at
`http://localhost:8081` (see [backend/README.md](../backend/README.md)).
Point it elsewhere with `TR_UI_API_URL=http://host:port npm run dev`.

To try the UI against the whole stack in local Kubernetes, see
[deploy/local/](../deploy/local/README.md).

## Test data

`npm run seed` fills an empty database with realistic, deterministic test
data through the public APIs, as a harness and a user would. It talks to the
local stack by default (`deploy/local/forward.sh`); for a server started with
`cargo run`, pass its ports:

```bash
npm run seed -- --ui http://localhost:8081 --api http://localhost:8080
```

It covers the cases the UI has to show, over 30 days from day 0
(2026-10-22) to day 29 (2026-11-20):

| What | Where |
|---|---|
| 30 nightly runs (`streaming-nightly`): 3 devices × 2 models × 2 bandwidths × 2 pipelines, warmup + 3 samples, `loaded` and `fps` series | every day |
| CI `smoke` runs on feature branches, scalar metrics only | weekdays |
| Regression in `pipeline=meshopt` load time, then fixed | days 12–17 |
| Shared environment change (backend `hub 2.4.0`) | from day 15 |
| Device detail change (Pixel on Android 16), marked in trends | from day 20 |
| Release tags `v1.4.0`, `v1.5.0` (runs tagged `release`) | days 0, 21 |
| Smaller downloads with `pipeline=draco` | from day 22 |
| New scenario version (`orbit-v2`): trends start over | from day 24 |
| Failed run with partial data and `harness.log` | day 9 |
| Invalid run (excluded from analysis by default) | day 13 |
| Corrected (superseded) case execution | day 27 |
| Experiment run on `feature/adaptive-lod` (manual), to compare | day 26 |
| Run left `running`; aborted by the stale timeout (1 h) | after day 29 |
| Incomplete loads (Pixel, city block, 20 Mbit/s): `load_time` missing, `derived.time_to_full` has no value | some samples |
| Aliases: device `pixel8-lab1` → `pixel-8-lab-1` (days 0–6), parameter `meshopt-v1` → `meshopt` (days 0–4) | |
| Derived metrics `throughput`, `time_to_90`, `time_to_full`, `steady_fps` (uses `derived.time_to_90`), `load_stall` | |
| Tags, run/test case/point-in-time annotations, attachments (log, Markdown, JSON, an SVG chart) | |

The script creates a temporary `write` token and revokes it at the end. It
refuses to seed a database twice; start from an empty one
(`deploy/local/down.sh`, `up.sh`).

## Image

```bash
docker build --tag track-record-frontend frontend
```

CI publishes it for `linux/amd64` and `linux/arm64` as
`ghcr.io/<owner>/track-record-frontend`
([ADR 0014](../docs/adr/0014-server-and-frontend-images.md)). The app is
built once on the build machine's platform; only the nginx stage differs.

nginx on port 8080 serves the app and proxies `/ui/api` to
`TR_UI_API_UPSTREAM` (default `http://server:8081`). The upstream host must
resolve when nginx starts.

## Translations

Every user-facing string comes from a locale file; components use
`useTranslation()` and never hard-code text. Keys are typed, so
`t('app.nmae')` fails the type check. Add a string by adding its key to
`src/i18n/locales/en/common.json` (and to every other language once there
are more; a test checks that all languages have the same keys).
