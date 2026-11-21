-- Versioned definitions (FR-7): content deduplication, version numbering and
-- immutability.
BEGIN;
SELECT plan(34);

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

-- Derived metrics: new version unless equal to the latest ---------------------

SELECT save_derived_metric('throughput', 'bytes / time', 'B/s') AS d1 \gset

SELECT is(save_derived_metric('throughput', 'bytes / time', 'B/s'), :d1::bigint,
    'saving an unchanged derived metric keeps its version');

SELECT save_derived_metric('throughput', 'bytes / time / 1000', 'kB/s') AS d2 \gset
SELECT save_derived_metric('throughput', 'bytes / time', 'B/s') AS d3 \gset

SELECT is((SELECT version FROM derived_metric_version WHERE id = :d3), 3,
    'returning to an earlier expression creates a new version');
SELECT results_eq(
    $$ SELECT version, expression FROM latest_derived_metric_version WHERE name = 'throughput' $$,
    $$ VALUES (3, 'bytes / time') $$,
    'latest_derived_metric_version shows the newest version');

-- Composition: pinned versions, no cycles (FR-17) ----------------------------

SELECT save_derived_metric('quality', 'clamp(score, 0, 1)', '') AS q1 \gset
SELECT save_derived_metric('quality_auc', 'auc(quality)', '', '', ARRAY[:q1]::bigint[]) AS auc1 \gset

SELECT is(save_derived_metric('quality_auc', 'auc(quality)', '', '', ARRAY[:q1]::bigint[]),
          :auc1::bigint,
    'saving an unchanged derived metric with the same pinned versions keeps its version');

SELECT save_derived_metric('quality', 'clamp(score, 0.001, 1)', '') AS q2 \gset

SELECT results_eq(
    format($$ SELECT uses_version_id FROM derived_metric_dependency
              WHERE derived_metric_version_id = %s $$, :auc1),
    format($$ VALUES (%s::bigint) $$, :q1),
    'a new version of a used metric does not change the metric built on it');

SELECT save_derived_metric('quality_auc', 'auc(quality)', '', '', ARRAY[:q2]::bigint[]) AS auc2 \gset

SELECT isnt(:auc2::bigint, :auc1::bigint,
    'pinning a newer version creates a new version, even with the same expression');
SELECT throws_ok(
    format($$ SELECT save_derived_metric('quality', 'quality_auc * 2', '', '', ARRAY[%s]::bigint[]) $$,
           :auc2),
    'TR007', NULL, 'a derived metric cannot depend on itself through another one');
SELECT throws_ok(
    format($$ SELECT save_derived_metric('both', 'a + b', '', '', ARRAY[%s, %s]::bigint[]) $$,
           :q1, :q2),
    '23505', NULL, 'a version pins at most one version of each derived metric');
SELECT throws_ok(
    $$ SELECT save_derived_metric('lost', 'missing', '', '', ARRAY[999999]::bigint[]) $$,
    '23503', NULL, 'a pinned version must exist');
SELECT throws_ok(
    format($$ INSERT INTO derived_metric_dependency
              SELECT %s, id, derived_metric_id FROM derived_metric_version WHERE id = %s $$,
           :q1, :auc1),
    '23514', NULL, 'a version can only use versions that existed before it');
SELECT throws_ok(
    $$ DELETE FROM derived_metric_dependency $$,
    'TR002', NULL, 'pinned versions cannot be removed');

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
