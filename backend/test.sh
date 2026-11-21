#!/usr/bin/env bash
# Runs the backend checks and tests against a throwaway database.
#
#   backend/test.sh                 fmt, clippy and all tests
#   backend/test.sh -- ingest       extra arguments go to `cargo test`
#
# Starts the database image (db/Dockerfile, which applies all migrations on
# first start) on a random local port. Every integration test then creates
# its own database from it as a template, so tests run in parallel and
# isolated. Needs Docker and a Rust toolchain.
set -euo pipefail

cd "$(dirname "$0")"
image=track-record-db
name="track-record-backend-test-$$"

docker build --quiet --tag "$image" ../db >/dev/null
docker run --detach --rm --name "$name" --env POSTGRES_PASSWORD=test \
    --publish 127.0.0.1::5432 "$image" \
    -c fsync=off -c full_page_writes=off -c synchronous_commit=off -c max_connections=500 >/dev/null
trap 'docker rm --force "$name" >/dev/null 2>&1 || true' EXIT

# During initialization the server only listens on its socket; TCP comes up
# once the migrations are applied and the final server runs.
for _ in $(seq 1 120); do
    if docker exec "$name" pg_isready --quiet --host=127.0.0.1 --username=postgres; then
        break
    fi
    sleep 0.5
done
docker exec "$name" pg_isready --quiet --host=127.0.0.1 --username=postgres \
    || { docker logs "$name"; echo "database did not start" >&2; exit 1; }
port=$(docker port "$name" 5432/tcp | head -n 1 | sed 's/.*://')

export TEST_DATABASE_URL="postgres://postgres:test@127.0.0.1:$port/postgres"
export TR_REQUIRE_DB=1

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked "$@"
