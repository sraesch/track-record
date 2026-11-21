# ADR 0010: Frontend scaffold

**Status:** proposed · **Date:** 2026-11-21

## Context

[ADR 0001](0001-technology-stack.md) fixes TypeScript (strict), React, Vite,
Vitest and react-i18next for the web UI, but left the repo layout, package
manager and tooling open. The UI talks to the backend's UI port (`/ui/api`,
[ADR 0008](0008-backend-service.md)), which sits behind the auth proxy and
needs no tokens.

## Decision

- **Layout:** a single Vite app in `frontend/`, next to `backend/` and `db/`.
  No monorepo tooling or workspaces.
- **Package manager:** npm, with the lockfile (`package-lock.json`) checked
  in. CI installs with `npm ci`. Node.js version in `frontend/.node-version`
  (24, the current LTS).
- **Scaffold:** `create-vite` `react-ts` template, demo content removed.
  TypeScript `strict` and `noUncheckedIndexedAccess` are set explicitly.
- **Lint:** oxlint, as the template ships it, with the `react`,
  `typescript`, `oxc`, `vitest` and `jsx-a11y` plugins; warnings fail the
  check.
- **i18n:** i18next with react-i18next, set up in `src/i18n/`. Locale files
  are JSON under `src/i18n/locales/<lng>/<namespace>.json` and are bundled,
  so translations are available on the first render and in tests. English
  is the only language for now. Translation keys are typed
  (`CustomTypeOptions`), so an unknown key fails `tsc`. A test checks that
  every language has the same keys.
- **Tests:** Vitest with jsdom and Testing Library (`@testing-library/react`,
  `jest-dom`, `user-event`), configured in `vite.config.ts`. Test files sit
  next to the code as `*.test.ts(x)`. Vitest globals are off; tests import
  from `vitest`.
- **One check command:** `frontend/test.sh` runs `npm ci`, lint, type check,
  tests and the production build. The CI job `frontend` runs it
  ([ADR 0005](0005-ci.md)).
- **Dev server:** Vite proxies `/ui/api` to a local backend
  (`http://localhost:8081`, override with `TR_UI_API_URL`), so the UI uses
  same-origin relative URLs in development and behind the ingress.
- **Image:** `frontend/Dockerfile` builds the app and serves it with
  unprivileged nginx on port 8080. nginx also proxies `/ui/api` to the
  backend UI port (`TR_UI_API_UPSTREAM`, default `http://server:8081`),
  without buffering so streamed results pass through (NFR-7), and falls back
  to `index.html` for client-side routes (FR-21).
- **Local stack:** `deploy/local/` runs database, server and UI on Docker
  Desktop Kubernetes with plain kustomize manifests (`up.sh`, `forward.sh`,
  `down.sh`). It is for trying the UI, not a deployment format; the Helm
  chart is still open.

## Consequences

- "No hard-coded user-facing strings" is enforced by review, not by lint:
  `eslint-plugin-i18next` loads as an oxlint JS plugin but reports nothing.
  Revisit when oxlint's JS plugin support matures, or switch to ESLint if
  this becomes a problem.
- Adding a language means adding a locale folder and registering it in
  `src/i18n/index.ts`; the key test catches missing translations.

## Left for later ADRs

Routing and URL state (FR-21) · API client generated from
`backend/openapi/ui.json` · data fetching and caching · charting library ·
component library and layout · ingress and publishing the UI image.
Component library, layout and routing are decided in
[ADR 0011](0011-ui-shell.md).
