-- Metrics: one definition per metric, which can be changed (FR-7, ADR 0018).
-- Uploads create metrics but never change them.
BEGIN;
SELECT plan(27);

\ir lib/fixtures.sql

-- Uploads: ensure_metric() -------------------------------------------------------

SELECT ensure_metric('load_time', 's', 'float', 'lower_is_better', 'Time to first frame') AS m1 \gset

SELECT results_eq(
    $$ SELECT unit, value_type, direction, description FROM metric WHERE name = 'load_time' $$,
    $$ VALUES ('s', 'float'::metric_value_type, 'lower_is_better'::metric_direction,
               'Time to first frame') $$,
    'an upload creates a metric that does not exist with the definition it sends');
SELECT is(ensure_metric('load_time', 's', 'float', 'lower_is_better', 'Time to first frame'),
    :m1::bigint, 'the same definition returns the same metric');
SELECT is(ensure_metric('load_time', 's', 'float', 'higher_is_better', 'Something else'),
    :m1::bigint, 'another direction or description returns the same metric');
SELECT results_eq(
    $$ SELECT direction, description FROM metric WHERE name = 'load_time' $$,
    $$ VALUES ('lower_is_better'::metric_direction, 'Time to first frame') $$,
    'an upload does not change the direction or description of a metric');
SELECT is((SELECT count(*) FROM metric WHERE name = 'load_time'), 1::bigint,
    'there is one definition per metric');

SELECT throws_ok(
    $$ SELECT ensure_metric('load_time', 'ms', 'float', 'lower_is_better') $$,
    'TR009', NULL, 'an upload with another unit is rejected');
SELECT throws_ok(
    $$ SELECT ensure_metric('load_time', 's', 'integer', 'lower_is_better') $$,
    'TR009', NULL, 'an upload with another value type is rejected');
SELECT is((SELECT unit FROM metric WHERE name = 'load_time'), 's',
    'a rejected upload leaves the unit as it was');
SELECT throws_ok(
    $$ SELECT ensure_metric(' ', 's', 'float', 'lower_is_better') $$,
    '23514', NULL, 'a metric needs a name');

-- Editing: save_metric() ----------------------------------------------------------

-- An older change time, set without the trigger that keeps it current.
ALTER TABLE metric DISABLE TRIGGER metric_touch;
UPDATE metric SET updated_at = '2026-01-01T00:00:00Z' WHERE id = :m1;
ALTER TABLE metric ENABLE TRIGGER metric_touch;

SELECT is(save_metric('load_time', 's', 'float', 'lower_is_better', 'Time to first frame'),
    :m1::bigint, 'saving a metric returns its id');
SELECT is((SELECT updated_at FROM metric WHERE id = :m1), '2026-01-01T00:00:00Z'::timestamptz,
    'saving unchanged content does not count as a change');

SELECT fx_new_run() AS run \gset
SELECT fx_new_case_execution(:'run') AS ce \gset
SELECT fx_new_sample(:'ce') AS sample \gset
INSERT INTO measurement VALUES (:'sample', :m1, 1500);

SELECT save_metric('load_time', 'ms', 'integer', 'higher_is_better', 'Time to the first frame') AS saved \gset
SELECT results_eq(
    $$ SELECT unit, value_type, direction, description FROM metric WHERE name = 'load_time' $$,
    $$ VALUES ('ms', 'integer'::metric_value_type, 'higher_is_better'::metric_direction,
               'Time to the first frame') $$,
    'saving changes the definition, also of a metric with results');
SELECT cmp_ok((SELECT updated_at FROM metric WHERE id = :m1), '>',
    '2026-01-01T00:00:00Z'::timestamptz, 'a change sets the change time');
SELECT results_eq(
    $$ SELECT r.value, m.unit FROM measurement r JOIN metric m ON m.id = r.metric_id $$,
    $$ VALUES (1500::double precision, 'ms') $$,
    'stored values stay as they are and are read in the new unit');
SELECT is(ensure_metric('load_time', 'ms', 'integer', 'lower_is_better'), :m1::bigint,
    'uploads now have to send the changed unit and value type');
SELECT is(save_metric('missing', 's', 'float', 'lower_is_better'), NULL,
    'saving a missing metric returns NULL and creates nothing');
SELECT is((SELECT count(*) FROM metric WHERE name = 'missing'), 0::bigint,
    'only uploads create metrics');

-- Renaming: rename_metric() --------------------------------------------------------

SELECT fx_metric('fps') AS m2 \gset

SELECT is(rename_metric('load_time', 'time_to_first_frame'), :m1::bigint,
    'renaming a metric returns its id');
SELECT results_eq(
    $$ SELECT m.name, r.value FROM measurement r JOIN metric m ON m.id = r.metric_id $$,
    $$ VALUES ('time_to_first_frame', 1500::double precision) $$,
    'results stay with the renamed metric');
SELECT is(rename_metric('load_time', 'other'), NULL,
    'renaming a missing metric returns NULL');
SELECT throws_ok(
    $$ SELECT rename_metric('fps', 'time_to_first_frame') $$,
    '23505', NULL, 'a metric cannot take the name of another one');
SELECT isnt(ensure_metric('load_time', 's', 'float', 'lower_is_better'), :m1::bigint,
    'an upload under the old name creates a new metric');

-- Deleting: delete_metric() --------------------------------------------------------

SELECT throws_ok(
    $$ SELECT delete_metric('time_to_first_frame') $$,
    'TR010', NULL, 'a metric with measurements cannot be deleted');
SELECT fx_new_series(:'sample', 2, 2, 'fps') AS fps_series \gset
SELECT throws_ok(
    $$ SELECT delete_metric('fps') $$,
    'TR010', NULL, 'a metric with series cannot be deleted');
SELECT ok(delete_metric('load_time'), 'a metric without results can be deleted');
SELECT is((SELECT count(*) FROM metric WHERE name = 'load_time'), 0::bigint,
    'the deleted metric is gone');
SELECT ok(NOT delete_metric('load_time'), 'deleting a missing metric returns false');

SELECT * FROM finish();
ROLLBACK;
