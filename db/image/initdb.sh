#!/usr/bin/env bash
# Run by the postgres entrypoint on first start with an empty data directory,
# against the temporary init server: applies all migrations to $POSTGRES_DB.
set -euo pipefail
PGUSER="$POSTGRES_USER" PGDATABASE="$POSTGRES_DB" track-record-migrate
