-- Shared dimensions: SUT versions, environments, devices, assets, test cases.
BEGIN;
SELECT plan(24);

-- Environments: deduplicated by fingerprint -----------------------------------

SELECT is(ensure_environment('{"gpu": "A100", "driver": "550"}'),
          ensure_environment('{"driver": "550", "gpu": "A100"}'),
    'identical environments are stored once, regardless of key order');
SELECT isnt(ensure_environment('{"gpu": "A100", "driver": "550"}'),
            ensure_environment('{"gpu": "A100", "driver": "560"}'),
    'different environments get different rows');
SELECT throws_ok(
    $$ SELECT ensure_environment('["not", "an", "object"]') $$,
    '23514', NULL, 'environment details must be a JSON object');

-- Client devices, SUT versions, assets ----------------------------------------

SELECT is(ensure_client_device('pixel-8-lab-1'), ensure_client_device('pixel-8-lab-1'),
    'client devices are identified by name');

SELECT is(ensure_sut_version('abc123', NULL, 'release'), ensure_sut_version('abc123', NULL, 'release'),
    'SUT versions with NULL fields are deduplicated');
SELECT isnt(ensure_sut_version('abc123', 'main', 'release'),
            ensure_sut_version('abc123', 'main', 'release', 'v1.0'),
    'a different tag is a different SUT version');

SELECT is(ensure_asset('city.glb', 'v2'), ensure_asset('city.glb', 'v2'),
    'assets are deduplicated by name and version');
SELECT throws_ok(
    $$ SELECT ensure_asset('city.glb', NULL, NULL) $$,
    '23514', NULL, 'an asset needs a version or a content hash');

-- Parameters ------------------------------------------------------------------

SELECT ok(is_flat_scalar_object('{"n": 1, "name": "x", "flag": true}'),
    'numbers, strings and booleans are valid parameters');
SELECT ok(NOT is_flat_scalar_object('{"nested": {"a": 1}}'), 'nested objects are rejected');
SELECT ok(NOT is_flat_scalar_object('{"list": [1, 2]}'), 'arrays are rejected');
SELECT ok(NOT is_flat_scalar_object('{"n": null}'), 'null values are rejected');
SELECT ok(NOT is_flat_scalar_object('[1, 2]'), 'parameters must be an object');

-- Test cases ------------------------------------------------------------------

SELECT ensure_scenario_version('load', '{"steps": 1}') AS sv \gset
SELECT ensure_asset('a.glb', 'v1') AS a1 \gset
SELECT ensure_asset('b.glb', 'v1') AS a2 \gset

SELECT ensure_test_case(:sv, ARRAY[:a1, :a2]::bigint[], '{"size": 10}') AS tc \gset

SELECT is(ensure_test_case(:sv, ARRAY[:a2, :a1, :a1]::bigint[], '{"size": 10}'), :tc::bigint,
    'asset order and duplicates do not create a new test case');
SELECT is((SELECT count(*)::int FROM test_case_asset WHERE test_case_id = :tc), 2,
    'test case is linked to each asset once');
SELECT isnt(ensure_test_case(:sv, ARRAY[:a1, :a2]::bigint[], '{"size": "10"}'), :tc::bigint,
    'parameter types matter: 10 and "10" are different test cases');
SELECT isnt(ensure_test_case(:sv, ARRAY[:a1]::bigint[], '{"size": 10}'), :tc::bigint,
    'a different asset set is a different test case');
SELECT throws_ok(
    format($$ SELECT ensure_test_case(%s, '{}', '{"size": {"mb": 10}}') $$, :sv),
    '23514', NULL, 'nested parameters are rejected');

-- Parameter units (FR-13a)
SELECT ensure_test_case(:sv, ARRAY[:a1, :a2]::bigint[], '{"size": 10}', '{"size": "MB"}') AS tc_mb \gset

SELECT isnt(:tc_mb::bigint, :tc::bigint,
    'a parameter unit makes a different test case');
SELECT is(ensure_test_case(:sv, ARRAY[:a1, :a2]::bigint[], '{"size": 10}', '{"size": "MB"}'),
          :tc_mb::bigint,
    'the same parameters and units return the same test case');
SELECT throws_ok(
    format($$ SELECT ensure_test_case(%s, '{}', '{"mode": "cold"}', '{"mode": "MB"}') $$, :sv),
    '23514', NULL, 'only numeric parameters can carry a unit');
SELECT throws_ok(
    format($$ SELECT ensure_test_case(%s, '{}', '{"size": 10}', '{"other": "MB"}') $$, :sv),
    '23514', NULL, 'a unit needs its parameter');
SELECT throws_ok(
    format($$ SELECT ensure_test_case(%s, '{}', '{"size": 10}', '{"size": 5}') $$, :sv),
    '23514', NULL, 'a unit is a symbol, not a number');

SELECT throws_ok(
    $$ UPDATE test_case SET parameters = '{}' $$,
    'TR001', NULL, 'test cases cannot be updated');

SELECT * FROM finish();
ROLLBACK;
