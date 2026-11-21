-- Unit catalogue and conversion (FR-13a).
BEGIN;
SELECT plan(11);

SELECT ok(abs(unit_conversion_factor('ms', 's') - 0.001) < 1e-12,
    'milliseconds convert to seconds');
SELECT ok(abs(unit_conversion_factor('h', 'min') - 60) < 1e-9,
    'hours convert to minutes');
SELECT ok(abs(unit_conversion_factor('Mbit/s', 'MB/s') - 0.125) < 1e-12,
    'megabits per second convert to megabytes per second');
SELECT ok(abs(unit_conversion_factor('MiB', 'KiB') - 1024) < 1e-9,
    'binary data sizes convert');
SELECT ok(abs(unit_conversion_factor('', '%') - 100) < 1e-9,
    'a dimensionless ratio converts to percent');

SELECT is(unit_conversion_factor('s', 'MB'), NULL,
    'units of different quantities are not converted');
SELECT is(unit_conversion_factor('triangles', 'count'), NULL,
    'a unit outside the catalogue is not converted');
SELECT is(unit_conversion_factor('triangles', 'triangles'), 1::double precision,
    'a unit always converts to itself');

SELECT lives_ok(
    $$ SELECT ensure_metric_version('triangle_count', 'triangles', 'integer', 'higher_is_better') $$,
    'a metric can use a unit outside the catalogue');

SELECT is((SELECT count(*)::int FROM unit
           WHERE symbol IN ('ms', 's', 'B', 'MB', 'MiB', 'Mbit/s', 'Hz', 'fps', '%', 'count')), 10,
    'common units for time, data size, data rate, frequency, ratio and count are predefined');

SELECT throws_ok(
    $$ UPDATE unit SET to_base = 1 $$,
    'TR001', NULL, 'catalogue units cannot be changed');

SELECT * FROM finish();
ROLLBACK;
