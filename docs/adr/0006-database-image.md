# ADR 0006: Database image and migration runner

**Status:** proposed · **Date:** 2026-11-21

## Context

Track Record runs on Kubernetes with Helm (NFR-1). The schema is a series of
plain SQL migrations ([ADR 0004](0004-database-schema.md)). So far they were
only applied to throwaway test databases. A deployment needs two things: a
database server with the schema, and a way to upgrade an existing database
when new migrations ship. Both must work without the Rust backend, which does
not exist yet.

## Decision

- **One image, `track-record-db`**, built from `db/Dockerfile` on top of the
  official `postgres:18` image. It contains the migrations and the migration
  runner. Default database: `track_record`.
- **Migration runner `db/migrate.sh`** (in the image:
  `track-record-migrate`). It applies pending migrations in version order,
  each in its own transaction together with its row in `schema_migration`
  (version, name, SHA-256 of the file, applied time). It connects through the
  standard libpq variables (`PGHOST`, `PGUSER`, `PGPASSWORD`, ...).
- **Refuse instead of guessing.** The runner stops if an applied migration
  has changed (checksum) or is missing (database newer than the image). This
  also enforces the rule that merged migrations are never edited.
- **Safe to run concurrently.** Every transaction takes the same advisory
  lock and checks again under it whether its migration is applied.
- **Two uses of the image:**
  - *Database server:* on first start with an empty data directory, an
    `initdb.d` script applies all migrations.
  - *Migration job:* `track-record-migrate` against any PostgreSQL 18
    database, for upgrades or for a database the image does not run (for
    example a managed one). The Helm chart will run it as a pre-install and
    pre-upgrade hook.
- **Tests use the same runner.** `db/test.sh` applies the migrations with
  `db/migrate.sh`; `db/image-test.sh` builds the image and checks first start,
  a no-op rerun, refusal of changed and missing migrations, and an upgrade.
  CI runs both.
- **Publishing** is decided in [ADR 0007](0007-image-publishing.md).

## Consequences

- `postgres:18` is a moving tag (minor versions and Debian updates). Builds are
  not reproducible to the byte; pin a digest if that matters.
- The `initdb.d` script only runs on an empty data directory. Restarting the
  server with a newer image does not migrate. Upgrades always need the
  migration job.
- `schema_migration` is our own table, not the format of a Rust migration
  library. If the backend later runs migrations itself (for example with
  `sqlx`), that decision has to take over or convert this table.
- The image is about 700 MB, almost all of it the base image.

## Left for later ADRs

The Helm chart, including the migration job.
