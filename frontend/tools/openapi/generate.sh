#!/usr/bin/env bash
# Generates the UI API types (src/api/schema.d.ts) from the backend's OpenAPI
# document (backend/openapi/ui.json), as ADR 0001 requires.
#
#   npm run generate:api              regenerate (in frontend/)
#   tools/openapi/generate.sh --check fail if schema.d.ts is out of date
#
# openapi-typescript needs the TypeScript 5 compiler API; the app uses
# TypeScript 7. So the generator has its own package here.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
frontend=$(cd "$here/../.." && pwd)
spec="$frontend/../backend/openapi/ui.json"
target="$frontend/src/api/schema.d.ts"

npm ci --prefix "$here" --no-audit --no-fund --silent

generate() {
    "$here/node_modules/.bin/openapi-typescript" "$spec" --output "$1" --silent
}

if [[ "${1:-}" == "--check" ]]; then
    tmp=$(mktemp)
    trap 'rm -f "$tmp"' EXIT
    generate "$tmp"
    if ! diff -q "$target" "$tmp" >/dev/null; then
        echo "src/api/schema.d.ts is out of date with backend/openapi/ui.json;" >&2
        echo "run 'npm run generate:api' in frontend/." >&2
        exit 1
    fi
else
    generate "$target"
fi
