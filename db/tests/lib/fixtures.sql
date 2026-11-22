-- Test fixtures, included by test files with \ir. They are created inside the
-- test's transaction and rolled back with it.

-- p_started_at is the measured start, p_created_at the upload time on the
-- server (tests set it to simulate an old upload).
CREATE FUNCTION fx_new_run(
    p_started_at timestamptz DEFAULT now(), p_created_at timestamptz DEFAULT now()
) RETURNS uuid
    LANGUAGE sql AS $$
    INSERT INTO run (id, suite_version_id, sut_version_id, shared_environment_id,
                     triggered_by, started_at, created_at)
    VALUES (gen_random_uuid(),
            ensure_suite_version('nightly-suite', '{"scenarios": ["load-model"]}'),
            ensure_sut_version('abc123', 'main', 'release'),
            ensure_environment('{"server": "backend-1"}'),
            'nightly', p_started_at, p_created_at)
    RETURNING id;
$$;

CREATE FUNCTION fx_test_case(p_size int DEFAULT 1) RETURNS bigint
    LANGUAGE sql AS $$
    SELECT ensure_test_case(
        ensure_scenario_version('load-model', '{"steps": ["open", "render"]}'),
        ARRAY[ensure_asset('engine.glb', 'v1')],
        jsonb_build_object('size_mb', p_size));
$$;

CREATE FUNCTION fx_new_case_execution(
    p_run_id uuid, p_device text DEFAULT 'pixel-8-lab-1', p_supersedes uuid DEFAULT NULL
) RETURNS uuid
    LANGUAGE sql AS $$
    INSERT INTO case_execution (id, run_id, test_case_id, client_device_id,
                                device_environment_id, supersedes_id)
    VALUES (gen_random_uuid(), p_run_id, fx_test_case(), ensure_client_device(p_device),
            ensure_environment('{"os": "Android 15"}'), p_supersedes)
    RETURNING id;
$$;

CREATE FUNCTION fx_new_sample(
    p_case_execution_id uuid, p_repetition int DEFAULT 0, p_warmup boolean DEFAULT false
) RETURNS uuid
    LANGUAGE sql AS $$
    INSERT INTO sample (id, case_execution_id, repetition, is_warmup)
    VALUES (gen_random_uuid(), p_case_execution_id, p_repetition, p_warmup)
    RETURNING id;
$$;

CREATE FUNCTION fx_metric(p_name text DEFAULT 'load_time') RETURNS bigint
    LANGUAGE sql AS $$
    SELECT ensure_metric(p_name, 's', 'float', 'lower_is_better');
$$;

-- Inserts a series declaring p_point_count points but writing p_written points,
-- then runs the deferred completeness check immediately. One series per sample
-- and metric, so further series of a sample need another p_metric.
CREATE FUNCTION fx_new_series(
    p_sample_id uuid, p_point_count int, p_written int DEFAULT NULL,
    p_metric text DEFAULT 'load_time'
) RETURNS uuid
    LANGUAGE plpgsql AS $$
DECLARE
    v_id  uuid := gen_random_uuid();
    v_key bigint;
BEGIN
    INSERT INTO series (id, sample_id, metric_id, x_dimension, x_unit, point_count)
    VALUES (v_id, p_sample_id, fx_metric(p_metric), 'time', 's', p_point_count)
    RETURNING key INTO v_key;

    INSERT INTO series_point (series_key, position, x, y)
    SELECT v_key, i, i * 0.5, i * 10
    FROM generate_series(0, coalesce(p_written, p_point_count) - 1) AS i;

    SET CONSTRAINTS series_complete IMMEDIATE;
    SET CONSTRAINTS series_complete DEFERRED;
    RETURN v_id;
END
$$;

-- Inserts a complete series with the given x values (y = position), then runs
-- the deferred check immediately.
CREATE FUNCTION fx_new_series_x(p_sample_id uuid, p_metric text, p_x real[]) RETURNS uuid
    LANGUAGE plpgsql AS $$
DECLARE
    v_id  uuid := gen_random_uuid();
    v_key bigint;
BEGIN
    INSERT INTO series (id, sample_id, metric_id, x_dimension, x_unit, point_count)
    VALUES (v_id, p_sample_id, fx_metric(p_metric), 'bytes', 'B', cardinality(p_x))
    RETURNING key INTO v_key;

    INSERT INTO series_point (series_key, position, x, y)
    SELECT v_key, ordinality - 1, x, ordinality - 1
    FROM unnest(p_x) WITH ORDINALITY AS t(x, ordinality);

    SET CONSTRAINTS series_complete IMMEDIATE;
    SET CONSTRAINTS series_complete DEFERRED;
    RETURN v_id;
END
$$;
