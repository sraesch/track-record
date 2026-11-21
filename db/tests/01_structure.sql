-- Schema structure: tables, views, types and immutability triggers.
BEGIN;
SELECT plan(30);

SELECT tables_are('public', ARRAY[
    'suite', 'suite_version', 'scenario', 'scenario_version', 'metric', 'metric_version',
    'derived_metric', 'derived_metric_version', 'derived_metric_dependency',
    'alias', 'alias_version', 'unit',
    'sut_version', 'environment', 'client_device', 'asset', 'test_case', 'test_case_asset',
    'run', 'case_execution', 'sample', 'measurement', 'series', 'series_point',
    'attachment', 'run_tag', 'test_case_tag', 'annotation', 'api_token',
    'schema_migration'  -- written by db/migrate.sh, not by a migration
]);

SELECT views_are('public', ARRAY[
    'current_alias', 'current_case_execution', 'latest_derived_metric_version'
]);

SELECT enum_has_labels('run_status', ARRAY['running', 'completed', 'failed', 'aborted']);
SELECT enum_has_labels('run_trigger', ARRAY['nightly', 'manual', 'ci']);
SELECT enum_has_labels('metric_value_type', ARRAY['float', 'integer', 'boolean']);
SELECT enum_has_labels('metric_direction', ARRAY['higher_is_better', 'lower_is_better']);
SELECT enum_has_labels('token_scope', ARRAY['read', 'annotate', 'write']);

-- Results use client-generated UUIDs (FR-3).
SELECT col_type_is(t, 'id', 'uuid')
FROM unnest(ARRAY['run', 'case_execution', 'sample', 'series', 'attachment']) AS t;

-- Series points are 32-bit floats (ADR 0002).
SELECT col_type_is('series_point', 'x', 'real');
SELECT col_type_is('series_point', 'y', 'real');

-- Alias values are JSON scalars, so parameter values keep their type.
SELECT col_type_is('alias', 'source_value', 'jsonb');

-- Every immutable table has the guard trigger.
SELECT has_trigger(t, 'immutable_rows')
FROM unnest(ARRAY[
    'suite_version', 'scenario_version', 'metric_version', 'derived_metric_version',
    'derived_metric_dependency', 'alias_version', 'unit', 'environment', 'test_case', 'case_execution', 'sample',
    'measurement', 'series', 'series_point', 'attachment'
]) AS t;

SELECT * FROM finish();
ROLLBACK;
