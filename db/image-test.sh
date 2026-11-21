#!/usr/bin/env bash
# Builds the database image and checks it: first start applies all migrations,
# the migration job is idempotent and refuses changed or missing migrations.
#   db/image-test.sh [image-tag]    default tag: track-record-db
set -euo pipefail
cd "$(dirname "$0")"
image=${1:-track-record-db}
container=track-record-db-image-test-$$
expected=$(find migrations -name '*.sql' | wc -l | tr -d ' ')

fail() { echo "FAIL: $*" >&2; exit 1; }
cleanup() { docker rm --force "$container" >/dev/null 2>&1 || true; }
trap cleanup EXIT

# Runs a command in a second container that shares the server's network, as a
# migration job in Kubernetes would.
job() {
    docker run --rm --network "container:$container" \
        --env PGHOST=127.0.0.1 --env PGUSER=postgres --env PGPASSWORD=test \
        --env PGDATABASE=track_record "$image" "$@"
}

echo "building $image"
docker build --quiet --tag "$image" . >/dev/null

echo "starting the server"
docker run --detach --name "$container" --env POSTGRES_PASSWORD=test "$image" >/dev/null
# TCP is only up once initialization is finished (the init server listens on
# the socket only).
for _ in $(seq 60); do
    if docker exec "$container" pg_isready --quiet --host=127.0.0.1 2>/dev/null; then break; fi
    sleep 1
done
docker exec "$container" pg_isready --quiet --host=127.0.0.1 \
    || { docker logs "$container" >&2; fail "server did not start"; }

applied=$(job psql --no-psqlrc --tuples-only --no-align \
    --command='SELECT count(*) FROM schema_migration')
[ "$applied" = "$expected" ] || fail "first start applied $applied of $expected migrations"
echo "ok: first start applied all $expected migrations"

output=$(job track-record-migrate)
[[ $output == *"0 migration(s) applied"* ]] || fail "second run was not a no-op: $output"
echo "ok: migration job on an up-to-date database applies nothing"

# expect_refusal <description> <expected error> <shell command run before the migration>
expect_refusal() {
    if output=$(job bash -c "$3 && track-record-migrate" 2>&1); then
        fail "$1 was accepted"
    fi
    [[ $output == *"$2"* ]] || fail "$1 failed for another reason: $output"
    echo "ok: $1 is refused"
}
expect_refusal "a changed migration" "changed after it was applied" \
    'echo "-- edited" >> "$TRACK_RECORD_MIGRATIONS"/0001_*.sql'
expect_refusal "a missing applied migration" "is applied but not in" \
    'rm "$TRACK_RECORD_MIGRATIONS"/0001_*.sql'

# Last, because it changes the database: an upgrade applies only the new one.
output=$(job bash -c 'echo "CREATE TABLE image_test_upgrade ();" \
    > "$TRACK_RECORD_MIGRATIONS/9999_image_test_upgrade.sql" && track-record-migrate')
[[ $output == *"1 migration(s) applied"* ]] || fail "upgrade did not apply one migration: $output"
echo "ok: an upgrade applies only the new migration"

echo "all image checks passed"
