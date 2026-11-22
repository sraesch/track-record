-- Definitions: content deduplication, version numbering and immutability of
-- the versioned ones (FR-7); derived metrics, which are changed in place.
BEGIN;
SELECT plan(48);

-- Scenarios: deduplicated by content -----------------------------------------

SELECT ensure_scenario_version('load', '{"steps": 1, "mode": "cold"}') AS s1 \gset

SELECT is(ensure_scenario_version('load', '{"steps": 1, "mode": "cold"}'), :s1::bigint,
    'same scenario definition returns the same version');
SELECT is(ensure_scenario_version('load', '{"mode": "cold", "steps": 1}'), :s1::bigint,
    'key order does not create a new scenario version');

SELECT ensure_scenario_version('load', '{"steps": 2, "mode": "cold"}') AS s2 \gset

SELECT isnt(:s2::bigint, :s1::bigint, 'changed definition creates a new scenario version');
SELECT is((SELECT version FROM scenario_version WHERE id = :s2), 2,
    'new scenario version is numbered 2');
SELECT is(ensure_scenario_version('load', '{"steps": 1, "mode": "cold"}'), :s1::bigint,
    'sending an old definition again returns its existing version');
SELECT ensure_scenario_version('render', '{"steps": 2, "mode": "cold"}') AS r1 \gset
SELECT is((SELECT version FROM scenario_version WHERE id = :r1), 1,
    'versions are numbered per scenario');

-- Suites and metrics follow the same rules ------------------------------------

SELECT is(ensure_suite_version('nightly', '{"scenarios": ["load"]}'),
          ensure_suite_version('nightly', '{"scenarios": ["load"]}'),
    'same suite definition returns the same version');

SELECT ensure_metric_version('load_time', 's', 'float', 'lower_is_better') AS m1 \gset

SELECT is(ensure_metric_version('load_time', 's', 'float', 'lower_is_better'), :m1::bigint,
    'same metric definition returns the same version');
SELECT ensure_metric_version('load_time', 'ms', 'float', 'lower_is_better') AS m2 \gset
SELECT is((SELECT version FROM metric_version WHERE id = :m2), 2,
    'changed unit creates metric version 2');
SELECT isnt(ensure_metric_version('load_time', 's', 'float', 'lower_is_better', 'Time to first frame'),
            :m1::bigint,
    'changed description creates a new metric version');

-- Derived metrics: one definition, which can be changed (ADR 0017) -------------

SELECT metric_id AS d1, created AS d1_created
FROM save_derived_metric('throughput', 'bytes / time', 'B/s') \gset

SELECT ok(:'d1_created'::boolean, 'saving a new derived metric creates it');
-- An older change time, set without the trigger that keeps it current.
ALTER TABLE derived_metric DISABLE TRIGGER derived_metric_touch;
UPDATE derived_metric SET updated_at = '2026-01-01T00:00:00Z' WHERE id = :d1;
ALTER TABLE derived_metric ENABLE TRIGGER derived_metric_touch;
SELECT results_eq(
    $$ SELECT metric_id, created FROM save_derived_metric('throughput', 'bytes / time', 'B/s') $$,
    format($$ VALUES (%s::bigint, false) $$, :d1),
    'saving a derived metric again returns the same one');
SELECT is((SELECT updated_at FROM derived_metric WHERE id = :d1), '2026-01-01T00:00:00Z'::timestamptz,
    'saving unchanged content does not count as a change');

SELECT save_derived_metric('throughput', 'bytes / time / 1000', 'kB/s', 'Download rate');
SELECT results_eq(
    $$ SELECT expression, unit, description FROM derived_metric WHERE name = 'throughput' $$,
    $$ VALUES ('bytes / time / 1000', 'kB/s', 'Download rate') $$,
    'saving other content changes the definition in place');
SELECT is((SELECT count(*)::int FROM derived_metric WHERE name = 'throughput'), 1,
    'a changed derived metric is still one row: there are no versions');
SELECT cmp_ok((SELECT updated_at FROM derived_metric WHERE id = :d1), '>',
              '2026-01-01T00:00:00Z'::timestamptz,
    'a change sets updated_at');

-- Composition: follows the expression, no cycles (FR-17) ------------------------

SELECT save_derived_metric('quality', 'clamp(score, 0, 1)', '');
SELECT save_derived_metric('smooth', 'moving_avg(series.fps, 1)', 'fps');
SELECT save_derived_metric('quality_auc', 'auc(derived.quality)', '', '', ARRAY['quality']);

SELECT results_eq(
    $$ SELECT u.name FROM derived_metric_dependency d
       JOIN derived_metric m ON m.id = d.derived_metric_id
       JOIN derived_metric u ON u.id = d.uses_derived_metric_id
       WHERE m.name = 'quality_auc' $$,
    $$ VALUES ('quality') $$,
    'the derived metrics an expression uses are recorded');

SELECT save_derived_metric('quality_auc', 'auc(derived.smooth) + derived.throughput', '', '',
                           ARRAY['smooth', 'throughput', 'smooth']);
SELECT results_eq(
    $$ SELECT u.name FROM derived_metric_dependency d
       JOIN derived_metric m ON m.id = d.derived_metric_id
       JOIN derived_metric u ON u.id = d.uses_derived_metric_id
       WHERE m.name = 'quality_auc' ORDER BY u.name $$,
    $$ VALUES ('smooth'), ('throughput') $$,
    'changing the expression replaces what it uses');

SELECT throws_ok(
    $$ SELECT save_derived_metric('smooth', 'derived.quality_auc * 2', '', '', ARRAY['quality_auc']) $$,
    'TR007', NULL, 'a derived metric cannot depend on itself through another one');
SELECT throws_ok(
    $$ SELECT save_derived_metric('smooth', 'derived.smooth', '', '', ARRAY['smooth']) $$,
    'TR007', NULL, 'a derived metric cannot use itself');
SELECT is((SELECT expression FROM derived_metric WHERE name = 'smooth'), 'moving_avg(series.fps, 1)',
    'a rejected save changes nothing');
SELECT throws_ok(
    $$ SELECT save_derived_metric('lost', 'derived.missing', '', '', ARRAY['missing']) $$,
    '23503', NULL, 'a used derived metric must exist');
SELECT throws_ok(
    $$ SELECT save_derived_metric('me', 'derived.me', '', '', ARRAY['me']) $$,
    '23503', NULL, 'a new derived metric cannot use itself');
SELECT throws_ok(
    format($$ INSERT INTO derived_metric_dependency VALUES (%s, %s) $$, :d1, :d1),
    '23514', NULL, 'the table itself rejects a metric that uses itself');

-- Renaming and deleting (ADR 0017) ---------------------------------------------

SELECT is(rename_derived_metric('throughput', 'rate'), :d1::bigint,
    'renaming returns the id');
SELECT results_eq(
    $$ SELECT u.name FROM derived_metric_dependency d
       JOIN derived_metric m ON m.id = d.derived_metric_id
       JOIN derived_metric u ON u.id = d.uses_derived_metric_id
       WHERE m.name = 'quality_auc' ORDER BY u.name $$,
    $$ VALUES ('rate'), ('smooth') $$,
    'a renamed metric stays used by the metrics built on it');
SELECT is(rename_derived_metric('nothing', 'other'), NULL,
    'renaming a missing metric returns NULL');
SELECT throws_ok(
    $$ SELECT rename_derived_metric('rate', 'smooth') $$,
    '23505', NULL, 'a derived metric cannot take a name that is in use');
SELECT throws_ok(
    $$ SELECT rename_derived_metric('rate', ' ') $$,
    '23514', NULL, 'a derived metric cannot get an empty name');

SELECT throws_ok(
    $$ SELECT delete_derived_metric('rate') $$,
    'TR008', 'derived metric rate is used by quality_auc; change or delete those first',
    'a derived metric that others use cannot be deleted');
SELECT ok(delete_derived_metric('quality_auc'), 'an unused derived metric can be deleted');
SELECT is_empty(
    $$ SELECT 1 FROM derived_metric WHERE name = 'quality_auc' $$,
    'the deleted metric is gone');
SELECT is((SELECT count(*)::int FROM derived_metric_dependency), 0,
    'what the deleted metric used is no longer recorded');
SELECT ok(delete_derived_metric('rate'), 'once nothing uses it, a metric can be deleted');
SELECT ok(NOT delete_derived_metric('rate'), 'deleting a missing metric returns false');

-- Aliases: values are JSON scalars -------------------------------------------

SELECT set_alias('client_device', '"pixel-8-a"', '"pixel-8"') AS a1 \gset

SELECT is(set_alias('client_device', '"pixel-8-a"', '"pixel-8"'), :a1::bigint,
    'setting the same alias target keeps its version');
SELECT results_eq(
    $$ SELECT target_value FROM current_alias WHERE source_value = '"pixel-8-a"' $$,
    $$ VALUES ('"pixel-8"'::jsonb) $$,
    'current_alias shows the alias');

SELECT set_alias('client_device', '"pixel-8-a"', '"pixel-8-lab"');
SELECT results_eq(
    $$ SELECT target_value, version FROM current_alias WHERE source_value = '"pixel-8-a"' $$,
    $$ VALUES ('"pixel-8-lab"'::jsonb, 2) $$,
    'changing the target creates version 2');

SELECT set_alias('client_device', '"pixel-8-a"', NULL);
SELECT is_empty(
    $$ SELECT 1 FROM current_alias WHERE source_value = '"pixel-8-a"' $$,
    'removing an alias hides it from current_alias');
SELECT is((SELECT count(*)::int FROM alias_version v JOIN alias a ON a.id = v.alias_id
           WHERE a.source_value = '"pixel-8-a"'), 3,
    'removal is recorded as a version, history is kept');

SELECT is(set_alias('client_device', '"never-existed"', NULL), NULL,
    'removing an unknown alias is a no-op');
SELECT throws_ok(
    $$ SELECT set_alias('client_device', '"same"', '"same"') $$,
    '23514', NULL, 'alias source and target must differ');

-- Case parameter values keep their JSON type.
SELECT set_alias('parameter:size', '10', '12');
SELECT set_alias('parameter:size', '"10"', '"ten"');
SELECT results_eq(
    $$ SELECT source_value, target_value FROM current_alias
       WHERE dimension = 'parameter:size' ORDER BY jsonb_typeof(source_value) $$,
    $$ VALUES ('10'::jsonb, '12'::jsonb), ('"10"'::jsonb, '"ten"'::jsonb) $$,
    'the number 10 and the string "10" are different alias sources');
SELECT throws_ok(
    $$ SELECT set_alias('parameter:size', '[10]', '12') $$,
    '23514', NULL, 'alias values must be JSON scalars');

-- Immutability ----------------------------------------------------------------

SELECT throws_ok(
    $$ UPDATE scenario_version SET definition = '{}' $$,
    'TR001', NULL, 'scenario versions cannot be updated');
SELECT throws_ok(
    $$ DELETE FROM metric_version $$,
    'TR002', NULL, 'metric versions cannot be deleted');
SELECT throws_ok(
    $$ TRUNCATE alias_version $$,
    'TR001', NULL, 'alias versions cannot be truncated');

SELECT set_config('track_record.deleting_run', 'on', true);
SELECT throws_ok(
    $$ DELETE FROM scenario_version $$,
    'TR002', NULL, 'definitions cannot be deleted even during run deletion');
SELECT set_config('track_record.deleting_run', 'off', true);

SELECT * FROM finish();
ROLLBACK;
