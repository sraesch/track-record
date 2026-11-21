-- Migration bookkeeping written by db/migrate.sh (ADR 0006).
BEGIN;
SELECT plan(3);

SELECT has_table('schema_migration', 'applied migrations are recorded');
SELECT is((SELECT count(*) FROM schema_migration),
          (SELECT max(version)::bigint FROM schema_migration),
    'every version from 1 up to the latest is recorded once');
SELECT is((SELECT count(*) FROM schema_migration WHERE checksum !~ '^[0-9a-f]{64}$'),
          0::bigint,
    'every applied migration has a SHA-256 checksum');

SELECT * FROM finish();
ROLLBACK;
