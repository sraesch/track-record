# Track Record database: PostgreSQL 18 with the schema migrations.
#
# Two uses (see docs/adr/0006-database-image.md):
# - Database server: on first start with an empty data directory, all
#   migrations are applied to $POSTGRES_DB. POSTGRES_PASSWORD is required, as
#   in the postgres image.
# - Migration job: `track-record-migrate` applies pending migrations to any
#   database given by the libpq variables (PGHOST, PGUSER, PGPASSWORD, ...).
#
# Build: docker build --tag track-record-db db
FROM postgres:18

LABEL org.opencontainers.image.title="track-record-db" \
      org.opencontainers.image.description="PostgreSQL 18 with the Track Record schema" \
      org.opencontainers.image.source="https://github.com/sraesch/track-record"

ENV POSTGRES_DB=track_record \
    TRACK_RECORD_MIGRATIONS=/opt/track-record/migrations

COPY --chmod=755 migrate.sh /usr/local/bin/track-record-migrate
COPY --chmod=755 image/initdb.sh /docker-entrypoint-initdb.d/10-track-record.sh
COPY migrations/ /opt/track-record/migrations/
