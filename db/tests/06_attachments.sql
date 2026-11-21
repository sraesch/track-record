-- Attachment metadata (ADR 0003): ownership consistency and immutability.
BEGIN;
\ir lib/fixtures.sql
SELECT plan(7);

SELECT fx_new_run() AS run \gset
SELECT fx_new_case_execution(:'run') AS ce \gset
SELECT fx_new_sample(:'ce') AS sample \gset
SELECT fx_new_run() AS other_run \gset
SELECT fx_new_case_execution(:'other_run') AS other_ce \gset

CREATE FUNCTION fx_attach(p_run uuid, p_ce uuid, p_sample uuid, p_hash bytea DEFAULT sha256('x'))
RETURNS uuid LANGUAGE sql AS $$
    INSERT INTO attachment (id, run_id, case_execution_id, sample_id, file_name,
                            content_type, size_bytes, sha256)
    VALUES (gen_random_uuid(), p_run, p_ce, p_sample, 'log.txt', 'text/plain', 1, p_hash)
    RETURNING id;
$$;

SELECT lives_ok(
    format($$ SELECT fx_attach(%L, NULL, NULL) $$, :'run'),
    'an attachment can belong to a run');
SELECT lives_ok(
    format($$ SELECT fx_attach(%L, %L, %L) $$, :'run', :'ce', :'sample'),
    'an attachment can belong to a sample of the run');
SELECT throws_ok(
    format($$ SELECT fx_attach(%L, %L, NULL) $$, :'run', :'other_ce'),
    '23503', NULL, 'the case execution must belong to the same run');
SELECT throws_ok(
    format($$ SELECT fx_attach(%L, %L, %L) $$, :'other_run', :'other_ce', :'sample'),
    '23503', NULL, 'the sample must belong to the same case execution');
SELECT throws_ok(
    format($$ SELECT fx_attach(%L, NULL, %L) $$, :'run', :'sample'),
    '23514', NULL, 'a sample attachment also names its case execution');
SELECT throws_ok(
    format($$ SELECT fx_attach(%L, NULL, NULL, '\x00'::bytea) $$, :'run'),
    '23514', NULL, 'sha256 must be 32 bytes');
SELECT throws_ok(
    $$ UPDATE attachment SET file_name = 'other.txt' $$,
    'TR001', NULL, 'attachments cannot be updated');

SELECT * FROM finish();
ROLLBACK;
