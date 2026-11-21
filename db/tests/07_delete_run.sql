-- Hard delete of a run (FR-10).
BEGIN;
\ir lib/fixtures.sql
SELECT plan(8);

-- A run with every kind of dependent row, and a second run that must survive.
SELECT fx_new_run() AS run \gset
SELECT fx_new_case_execution(:'run') AS ce \gset
SELECT fx_new_sample(:'ce') AS sample \gset
INSERT INTO measurement VALUES (:'sample', fx_metric(), 1.0);
SELECT fx_new_series(:'sample', 100);
INSERT INTO attachment (id, run_id, case_execution_id, file_name, content_type, size_bytes, sha256)
VALUES (gen_random_uuid(), :'run', :'ce', 'trace.json', 'application/json', 1, sha256('t'));
INSERT INTO run_tag VALUES (:'run', 'baseline');
INSERT INTO annotation (run_id, body) VALUES (:'run', 'first run on new hardware');

SELECT fx_new_run() AS kept \gset
SELECT fx_new_case_execution(:'kept') AS kept_ce \gset
SELECT fx_new_series(fx_new_sample(:'kept_ce'), 10);

SELECT is(delete_run(:'run'), true, 'delete_run returns true for an existing run');

SELECT is_empty(
    format($$
        SELECT 'run' FROM run WHERE id = %1$L
        UNION ALL SELECT 'case_execution' FROM case_execution WHERE run_id = %1$L
        UNION ALL SELECT 'sample' FROM sample WHERE id = %2$L
        UNION ALL SELECT 'measurement' FROM measurement WHERE sample_id = %2$L
        UNION ALL SELECT 'series' FROM series WHERE sample_id = %2$L
        UNION ALL SELECT 'attachment' FROM attachment WHERE run_id = %1$L
        UNION ALL SELECT 'run_tag' FROM run_tag WHERE run_id = %1$L
        UNION ALL SELECT 'annotation' FROM annotation WHERE run_id = %1$L
    $$, :'run', :'sample'),
    'the run and all its dependent rows are gone');
SELECT is((SELECT count(*)::int FROM series_point), 10,
    'only the points of the deleted run are removed');
SELECT is((SELECT count(*)::int FROM case_execution WHERE run_id = :'kept'), 1,
    'other runs are untouched');
SELECT isnt((SELECT count(*)::int FROM test_case), 0,
    'shared definitions and test cases are kept');

SELECT is(delete_run(gen_random_uuid()), false, 'delete_run returns false for an unknown run');
SELECT is(current_setting('track_record.deleting_run'), 'off',
    'the deletion flag is reset afterwards');
SELECT throws_ok(
    format($$ DELETE FROM case_execution WHERE run_id = %L $$, :'kept'),
    'TR002', NULL, 'direct deletes are still rejected after delete_run');

SELECT * FROM finish();
ROLLBACK;
