-- Samples, measurements and series: idempotency (FR-3), immutability (FR-6),
-- series completeness (ADR 0002) and x order.
BEGIN;
\ir lib/fixtures.sql
SELECT plan(21);

SELECT fx_new_run() AS run \gset
SELECT fx_new_case_execution(:'run') AS ce \gset
SELECT fx_new_sample(:'ce', 0, true) AS warmup \gset
SELECT fx_new_sample(:'ce', 1) AS sample \gset

-- Samples ---------------------------------------------------------------------

SELECT throws_ok(
    format($$ INSERT INTO sample (id, case_execution_id, repetition) VALUES (%L, %L, 5) $$,
           :'sample', :'ce'),
    '23505', NULL, 'a retried sample upload hits the existing id');
SELECT throws_ok(
    format($$ SELECT fx_new_sample(%L, 1) $$, :'ce'),
    '23505', NULL, 'repetition numbers are unique per case execution');
SELECT is((SELECT is_warmup FROM sample WHERE id = :'warmup'), true,
    'warmup flag is stored');

-- Measurements ----------------------------------------------------------------

SELECT lives_ok(
    format($$ INSERT INTO measurement VALUES (%L, fx_metric(), 1.25) $$, :'sample'),
    'a finite measurement is stored');
SELECT throws_ok(
    format($$ INSERT INTO measurement VALUES (%L, fx_metric(), 2.0) $$, :'sample'),
    '23505', NULL, 'one value per sample and metric');
SELECT throws_ok(
    format($$ INSERT INTO measurement VALUES (%L, fx_metric(), 'NaN') $$, :'warmup'),
    '23514', NULL, 'NaN is rejected');
SELECT throws_ok(
    format($$ INSERT INTO measurement VALUES (%L, fx_metric(), 'Infinity') $$, :'warmup'),
    '23514', NULL, 'Infinity is rejected');

SELECT throws_ok(
    $$ UPDATE measurement SET value = 0 $$,
    'TR001', NULL, 'measurements cannot be updated');
SELECT throws_ok(
    format($$ DELETE FROM sample WHERE id = %L $$, :'sample'),
    'TR002', NULL, 'samples cannot be deleted directly');

-- Series ----------------------------------------------------------------------

SELECT lives_ok(
    format($$ SELECT fx_new_series(%L, 3) $$, :'sample'),
    'a complete series is accepted');
SELECT throws_ok(
    format($$ SELECT fx_new_series(%L, 3, 2, 'fps') $$, :'sample'),
    'TR005', NULL, 'a series with missing points is rejected at commit');
SELECT lives_ok(
    format($$ SELECT fx_new_series(%L, 0, 0, 'empty') $$, :'sample'),
    'an empty series is accepted');

SELECT key AS series_key FROM series WHERE sample_id = :'sample' AND point_count = 3 \gset

SELECT throws_ok(
    format($$ INSERT INTO series_point VALUES (%s, 3, 1.5, 30) $$, :series_key),
    'TR005', NULL, 'points cannot be added beyond point_count');
SELECT throws_ok(
    format($$ INSERT INTO series_point VALUES (%s, 1, 0.5, 10) $$, :series_key),
    '23505', NULL, 'point positions are unique');
SELECT throws_ok(
    format($$ SELECT fx_new_series_x(%L, 'load_time', ARRAY[0, 1, 2]) $$, :'sample'),
    '23505', NULL, 'one series per sample and metric, whatever its x-dimension');

-- x order: never decreases, may repeat (e.g. bytes received while idle).
SELECT lives_ok(
    format($$ SELECT fx_new_series_x(%L, 'plateau', ARRAY[0, 100, 100, 250]) $$, :'sample'),
    'repeated x values are accepted');
SELECT throws_ok(
    format($$ SELECT fx_new_series_x(%L, 'backwards', ARRAY[0, 100, 50]) $$, :'sample'),
    'TR006', NULL, 'decreasing x values are rejected at commit');

SELECT results_eq(
    format($$ SELECT position, x, y FROM series_point WHERE series_key = %s ORDER BY position $$,
           :series_key),
    $$ VALUES (0, 0::real, 0::real), (1, 0.5::real, 10::real), (2, 1::real, 20::real) $$,
    'points are stored in order');

SELECT throws_ok(
    $$ UPDATE series_point SET y = 0 $$,
    'TR001', NULL, 'series points cannot be updated');
SELECT throws_ok(
    $$ DELETE FROM series $$,
    'TR002', NULL, 'series cannot be deleted directly');
SELECT throws_ok(
    $$ TRUNCATE series_point $$,
    'TR001', NULL, 'series points cannot be truncated');

SELECT * FROM finish();
ROLLBACK;
