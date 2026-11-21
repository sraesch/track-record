#!/usr/bin/env bash
# Runs inside the test container: starts a throwaway PostgreSQL, applies all
# migrations, then runs the pgTAP tests with pg_prove.
# Optional arguments: test files to run (default: all of /db/tests/*.sql).
set -euo pipefail

export PGDATA=/tmp/pgdata PGDATABASE=track_record PGUSER=postgres

initdb --auth=trust --username=postgres --encoding=UTF8 >/dev/null
pg_ctl --wait --log=/tmp/postgres.log \
    --options="-c listen_addresses='' -c fsync=off -c full_page_writes=off" start >/dev/null
createdb track_record

for migration in /db/migrations/*.sql; do
    psql --no-psqlrc --quiet --single-transaction -v ON_ERROR_STOP=1 \
        --file="$migration" >/dev/null
done

psql --no-psqlrc --quiet -v ON_ERROR_STOP=1 --command='CREATE EXTENSION pgtap' >/dev/null

if [ "$#" -gt 0 ]; then
    pg_prove --failures "$@"
else
    pg_prove --failures /db/tests/*.sql
fi
