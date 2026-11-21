# Frontend

Web UI for Track Record: TypeScript (strict), React, Vite, Vitest and
react-i18next. Setup decisions are in
[ADR 0010](../docs/adr/0010-frontend-scaffold.md).

## Layout

```
frontend/
  index.html            Vite entry
  src/main.tsx          Bootstraps React and i18n
  src/App.tsx           App shell
  src/i18n/             i18next setup, typed keys, locale files
    locales/en/         English translations (JSON, one file per namespace)
  src/test/setup.ts     Vitest setup: jest-dom matchers, i18n, cleanup
  vite.config.ts        Vite and Vitest config, dev proxy to the backend
  test.sh               lint, type check, tests and build (what CI runs)
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

## Translations

Every user-facing string comes from a locale file; components use
`useTranslation()` and never hard-code text. Keys are typed, so
`t('app.nmae')` fails the type check. Add a string by adding its key to
`src/i18n/locales/en/common.json` (and to every other language once there
are more; a test checks that all languages have the same keys).
