# ADR 0012: Frontend data access

**Status:** proposed · **Date:** 2026-11-21

## Context

[ADR 0001](0001-technology-stack.md) requires the frontend API types to be
generated from the OpenAPI spec; how, and how the UI fetches and caches data,
was open ([ADR 0010](0010-frontend-scaffold.md),
[ADR 0011](0011-ui-shell.md)). Views are live: they reflect later runs,
alias changes and invalidations (FR-21). The UI only calls the UI port
(`/ui/api`).

## Decision

- **Generated types:** `openapi-typescript` turns `backend/openapi/ui.json`
  into `frontend/src/api/schema.d.ts`, which is checked in.
  `npm run generate:api` regenerates it; `frontend/test.sh` (and so CI)
  fails when it is out of date, the same way the backend checks its
  OpenAPI documents.
- **Generator in its own package:** `openapi-typescript` needs the
  TypeScript 5 compiler API, which TypeScript 7 no longer has, and npm
  cannot give a peer dependency its own copy. So it lives in
  `frontend/tools/openapi/` with its own `package.json`, lockfile and
  TypeScript 5. The app keeps TypeScript 7.
- **Client:** `openapi-fetch`, a thin typed `fetch` wrapper
  (`src/api/client.ts`). Paths, query parameters and responses are checked
  against the generated types. Errors become `ApiError` with the backend's
  problem `code` and `detail`.
- **Fetching and caching:** TanStack Query. Query keys start with the
  resource (`['runs', 'list', query]`). Data is fresh for 30 s and is
  refetched when the window regains focus or a view mounts, which keeps
  views live. Cursor-paged lists use infinite queries ("Load more").
- **View state stays in the URL** (FR-21); queries are derived from it.
  Loaded pages beyond the first are not in the URL: a link reproduces the
  filters, not how far someone scrolled.
- **Tests** stub `fetch` (`src/test/api.ts`): handlers by path, every
  request recorded. Every test starts with empty answers, so no test can
  reach a real server.

## Consequences

- After changing an endpoint in the backend, regenerate the OpenAPI
  documents there and `npm run generate:api` here.
- Long operations (NFR-7: comparisons, exports) will need streaming or
  job polling on top of this; that is decided with the first such view.
