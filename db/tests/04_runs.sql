-- Run lifecycle (FR-1, FR-2), validity (FR-9) and corrections (FR-6).
BEGIN;
\ir lib/fixtures.sql
SELECT plan(25);

-- Status lifecycle ------------------------------------------------------------

SELECT throws_ok(
    $$ INSERT INTO run (id, suite_version_id, sut_version_id, shared_environment_id,
                        triggered_by, started_at, status, ended_at)
       VALUES (gen_random_uuid(), ensure_suite_version('s', '{}'), ensure_sut_version('c'),
               ensure_environment('{}'), 'manual', now(), 'completed', now()) $$,
    'TR003', NULL, 'a run cannot be created as completed');

SELECT fx_new_run() AS run \gset

SELECT results_eq(
    format($$ SELECT status, ended_at FROM run WHERE id = %L $$, :'run'),
    $$ VALUES ('running'::run_status, NULL::timestamptz) $$,
    'a new run is running without end time');

SELECT lives_ok(
    format($$ UPDATE run SET status = 'completed' WHERE id = %L $$, :'run'),
    'running -> completed is allowed');
SELECT isnt((SELECT ended_at FROM run WHERE id = :'run'), NULL,
    'ended_at is set when the run finishes');

SELECT throws_ok(
    format($$ UPDATE run SET status = 'failed' WHERE id = %L $$, :'run'),
    'TR003', NULL, 'completed -> failed is rejected');
SELECT throws_ok(
    format($$ UPDATE run SET status = 'running' WHERE id = %L $$, :'run'),
    'TR003', NULL, 'completed -> running is rejected');
SELECT throws_ok(
    format($$ UPDATE run SET ended_at = ended_at + interval '1 hour' WHERE id = %L $$, :'run'),
    'TR001', NULL, 'ended_at cannot change after the run finished');
SELECT throws_ok(
    format($$ UPDATE run SET started_at = now() - interval '1 day' WHERE id = %L $$, :'run'),
    'TR001', NULL, 'started_at cannot change');
SELECT throws_ok(
    format($$ UPDATE run SET sut_version_id = ensure_sut_version('other') WHERE id = %L $$, :'run'),
    'TR001', NULL, 'the SUT version of a run cannot change');

-- Validity --------------------------------------------------------------------

SELECT throws_ok(
    format($$ UPDATE run SET is_valid = false WHERE id = %L $$, :'run'),
    '23514', NULL, 'marking a run invalid requires a reason');
SELECT lives_ok(
    format($$ UPDATE run SET is_valid = false, invalid_reason = 'test upload' WHERE id = %L $$, :'run'),
    'a run can be marked invalid with a reason');
SELECT throws_ok(
    format($$ UPDATE run SET is_valid = true WHERE id = %L $$, :'run'),
    '23514', NULL, 'a valid run cannot keep an invalid reason');
SELECT lives_ok(
    format($$ UPDATE run SET is_valid = true, invalid_reason = NULL WHERE id = %L $$, :'run'),
    'a run can be marked valid again');

-- Case executions and corrections ---------------------------------------------

SELECT fx_new_run() AS open_run \gset
SELECT fx_new_case_execution(:'open_run') AS ce \gset
UPDATE run SET status = 'completed' WHERE id = :'open_run';

SELECT throws_ok(
    format($$ SELECT fx_new_case_execution(%L) $$, :'open_run'),
    'TR004', NULL, 'new case executions need a running run');

SELECT fx_new_case_execution(:'open_run', 'pixel-8-lab-1', :'ce') AS fix \gset

SELECT results_eq(
    format($$ SELECT id FROM current_case_execution WHERE run_id = %L $$, :'open_run'),
    format($$ VALUES (%L::uuid) $$, :'fix'),
    'a correction can be added to a finished run and replaces the original');
SELECT is((SELECT count(*)::int FROM case_execution WHERE run_id = :'open_run'), 2,
    'the original case execution is kept');
SELECT throws_ok(
    format($$ SELECT fx_new_case_execution(%L, 'pixel-8-lab-1', %L) $$, :'open_run', :'ce'),
    '23505', NULL, 'a case execution can be superseded only once');
SELECT throws_ok(
    format($$ SELECT fx_new_case_execution(%L, 'other-device', %L) $$, :'open_run', :'fix'),
    '23503', NULL, 'a correction must be for the same test case and device');

SELECT throws_ok(
    format($$ UPDATE case_execution SET started_at = now() WHERE id = %L $$, :'ce'),
    'TR001', NULL, 'case executions cannot be updated');
SELECT throws_ok(
    format($$ DELETE FROM run WHERE id = %L $$, :'run'),
    'TR002', NULL, 'runs cannot be deleted directly');

-- Stale runs: no upload for the timeout, in server time ----------------------

-- Measured three days ago, uploaded just now (late upload or import).
SELECT fx_new_run(now() - interval '3 days') AS imported \gset
-- Uploaded three hours ago, nothing since.
SELECT fx_new_run(now() - interval '3 hours', now() - interval '3 hours') AS stale \gset
-- Uploaded three hours ago, then a new case execution, attachment or series.
SELECT fx_new_run(now() - interval '3 hours', now() - interval '3 hours') AS active \gset
SELECT fx_new_case_execution(:'active');
SELECT fx_new_run(now() - interval '3 hours', now() - interval '3 hours') AS attaching \gset
INSERT INTO attachment (id, run_id, file_name, content_type, size_bytes, sha256)
VALUES (gen_random_uuid(), :'attaching', 'trace.json', 'application/json', 1, sha256('x'));
SELECT fx_new_run(now() - interval '3 hours', now() - interval '3 hours') AS streaming \gset
INSERT INTO case_execution (id, run_id, test_case_id, client_device_id,
                            device_environment_id, created_at)
VALUES (gen_random_uuid(), :'streaming', fx_test_case(), ensure_client_device('pixel-8-lab-1'),
        ensure_environment('{"os": "Android 15"}'), now() - interval '3 hours')
RETURNING id AS streaming_ce \gset
INSERT INTO sample (id, case_execution_id, repetition, created_at)
VALUES (gen_random_uuid(), :'streaming_ce', 0, now() - interval '3 hours')
RETURNING id AS streaming_sample \gset
SELECT fx_new_series(:'streaming_sample', 3);
SELECT fx_new_run() AS fresh \gset

SELECT results_eq(
    $$ SELECT abort_stale_runs(interval '2 hours') $$,
    format($$ VALUES (%L::uuid) $$, :'stale'),
    'only the run without a recent upload is aborted');
SELECT results_eq(
    format($$ SELECT status FROM run WHERE id = %L $$, :'stale'),
    $$ VALUES ('aborted'::run_status) $$,
    'stale run is marked aborted');
SELECT isnt((SELECT ended_at FROM run WHERE id = :'stale'), NULL,
    'aborted run gets an end time');
SELECT results_eq(
    format($$ SELECT status FROM run WHERE id = %L $$, :'imported'),
    $$ VALUES ('running'::run_status) $$,
    'a run measured days ago but uploaded now is not stale');
SELECT is(
    (SELECT count(*)::int FROM run
     WHERE id IN (:'active', :'attaching', :'streaming', :'fresh') AND status = 'running'),
    4, 'a recent case execution, attachment or series keeps a run running');

SELECT * FROM finish();
ROLLBACK;
