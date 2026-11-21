#!/usr/bin/env bash
# Applies the pending database migrations (NNNN_name.sql) in order.
#
# Each migration runs in its own transaction together with its row in
# schema_migration, so a failed migration leaves nothing behind. Migrations
# that are already applied are skipped. It is an error if an applied migration
# has changed or is missing here (an image older than the database).
# Concurrent runs are safe: each transaction takes an advisory lock.
#
# Connection: the standard libpq variables (PGHOST, PGPORT, PGDATABASE, PGUSER,
# PGPASSWORD, ...). Migrations: $TRACK_RECORD_MIGRATIONS, by default the
# migrations/ directory next to this script.
set -euo pipefail

dir=${TRACK_RECORD_MIGRATIONS:-$(cd "$(dirname "$0")" && pwd)/migrations}
# Advisory lock key, the same for every run of this script.
lock=4711202611

die() { echo "track-record-migrate: $*" >&2; exit 1; }
run_psql() { psql --no-psqlrc --quiet --set=ON_ERROR_STOP=1 "$@"; }
sha256() {
    if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi \
        | cut -d ' ' -f 1
}

# Migrations in this directory, indexed by version.
shopt -s nullglob
files=("$dir"/*.sql)
[ "${#files[@]}" -gt 0 ] || die "no migrations found in $dir"
paths=() names=() sums=() versions=()
for path in "${files[@]}"; do
    base=$(basename "$path")
    [[ $base =~ ^([0-9]{4})_([a-z0-9_]+)\.sql$ ]] \
        || die "$base does not match NNNN_name.sql"
    version=$((10#${BASH_REMATCH[1]}))
    [ -z "${paths[$version]:-}" ] || die "version $version is used twice"
    paths[$version]=$path
    names[$version]=${BASH_REMATCH[2]}
    sums[$version]=$(sha256 "$path")
    versions+=("$version")
done

run_psql --single-transaction --file=- <<SQL
\o /dev/null
SET LOCAL client_min_messages = warning;
SELECT pg_advisory_xact_lock($lock);
CREATE TABLE IF NOT EXISTS schema_migration (
    version    integer PRIMARY KEY,
    name       text NOT NULL,
    checksum   text NOT NULL,  -- SHA-256 of the migration file, hex
    applied_at timestamptz NOT NULL DEFAULT now()
);
SQL

# Applied migrations must be unchanged and present here.
applied=()
while read -r version name sum; do
    [ -n "$version" ] || continue
    [ -n "${paths[$version]:-}" ] \
        || die "migration $version ($name) is applied but not in $dir; is the database newer than these migrations?"
    [ "$sum" = "${sums[$version]}" ] \
        || die "migration $version (${names[$version]}) changed after it was applied"
    applied[$version]=1
done < <(run_psql --tuples-only --no-align --field-separator=' ' \
    --command='SELECT version, name, checksum FROM schema_migration ORDER BY version')

count=0
for version in "${versions[@]}"; do
    [ -z "${applied[$version]:-}" ] || continue
    # Checked again under the lock: a concurrent run may have applied it.
    # Query results of the migration itself are discarded.
    result=$(run_psql --single-transaction --file=- \
        --set=version="$version" --set=name="${names[$version]}" \
        --set=checksum="${sums[$version]}" --set=path="${paths[$version]}" <<SQL
\o /dev/null
SELECT pg_advisory_xact_lock($lock);
SELECT EXISTS (SELECT 1 FROM schema_migration WHERE version = :version) AS done \gset
\if :done
\echo skipped
\else
\i :path
INSERT INTO schema_migration (version, name, checksum) VALUES (:version, :'name', :'checksum');
\echo applied
\endif
SQL
    )
    if [ "$result" = applied ]; then
        echo "applied ${paths[$version]##*/}"
        count=$((count + 1))
    else
        echo "skipped ${paths[$version]##*/}: applied by a concurrent run"
    fi
done

echo "database is up to date: version ${versions[${#versions[@]} - 1]}, $count migration(s) applied"
