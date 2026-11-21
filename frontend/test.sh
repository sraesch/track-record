#!/usr/bin/env bash
# Runs the frontend checks: install from the lockfile, lint, type check,
# tests and production build. Needs Node.js (version in .node-version).
#
#   frontend/test.sh                  everything
#   frontend/test.sh -- App           extra arguments go to `vitest run`
set -euo pipefail

cd "$(dirname "$0")"

if [[ "${1:-}" == "--" ]]; then
    shift
fi

npm ci --no-audit --no-fund
tools/openapi/generate.sh --check
npm run lint
npm run typecheck
npm test -- "$@"
npm run build
