-- Shared dimensions that runs and case executions reference: SUT versions,
-- environments, client devices, assets and test cases. All are deduplicated
-- and immutable; the ensure_* functions return the matching row's id.

-- SUT versions ---------------------------------------------------------------

CREATE TABLE sut_version (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    commit       text NOT NULL CHECK (btrim(commit) <> ''),
    branch       text,
    build_config text,
    tag          text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (commit, branch, build_config, tag)
);

-- Environments ---------------------------------------------------------------
-- Used in two roles: the shared environment of a run and the device details of
-- a case execution. Identical details are stored once (fingerprint).

CREATE TABLE environment (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    details     jsonb NOT NULL CHECK (jsonb_typeof(details) = 'object'),
    fingerprint bytea NOT NULL UNIQUE GENERATED ALWAYS AS (jsonb_sha256(details)) STORED,
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- Client devices -------------------------------------------------------------
-- Identified by the stable name the harness sends. Details live in
-- environment and are recorded per case execution, so a detail change marks
-- the trend without splitting it.

CREATE TABLE client_device (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text NOT NULL UNIQUE CHECK (btrim(name) <> ''),
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Assets ---------------------------------------------------------------------

CREATE TABLE asset (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name         text NOT NULL CHECK (btrim(name) <> ''),
    version      text,
    content_hash text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    CHECK (version IS NOT NULL OR content_hash IS NOT NULL),
    UNIQUE NULLS NOT DISTINCT (name, version, content_hash)
);

-- Test cases -----------------------------------------------------------------
-- Scenario version + assets + parameter values: the unit compared across runs.
-- ("case" is an SQL keyword, hence test_case.) Numeric parameters can carry a
-- unit (FR-13a), e.g. parameters {"bandwidth": 10} with parameter_units
-- {"bandwidth": "Mbit/s"}. The fingerprint covers all parts, units included,
-- and is computed by test_case_fingerprint().

CREATE TABLE test_case (
    id                  bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scenario_version_id bigint NOT NULL REFERENCES scenario_version,
    parameters          jsonb NOT NULL DEFAULT '{}' CHECK (is_flat_scalar_object(parameters)),
    parameter_units     jsonb NOT NULL DEFAULT '{}',
    fingerprint         bytea NOT NULL UNIQUE,
    created_at          timestamptz NOT NULL DEFAULT now(),
    CHECK (are_parameter_units(parameter_units, parameters))
);

CREATE INDEX test_case_scenario_version_idx ON test_case (scenario_version_id);
CREATE INDEX test_case_parameters_idx ON test_case USING gin (parameters jsonb_path_ops);

CREATE TABLE test_case_asset (
    test_case_id bigint NOT NULL REFERENCES test_case,
    asset_id     bigint NOT NULL REFERENCES asset,
    PRIMARY KEY (test_case_id, asset_id)
);

CREATE INDEX test_case_asset_asset_idx ON test_case_asset (asset_id);

-- Immutability ---------------------------------------------------------------

SELECT make_immutable(t, false)
FROM unnest(ARRAY[
    'sut_version', 'environment', 'client_device', 'asset', 'test_case', 'test_case_asset'
]::regclass[]) AS t;

-- Functions ------------------------------------------------------------------

CREATE FUNCTION ensure_sut_version(
    p_commit text, p_branch text DEFAULT NULL, p_build_config text DEFAULT NULL, p_tag text DEFAULT NULL
) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    INSERT INTO sut_version (commit, branch, build_config, tag)
    VALUES (p_commit, p_branch, p_build_config, p_tag)
    ON CONFLICT (commit, branch, build_config, tag) DO NOTHING
    RETURNING id INTO v_id;

    IF v_id IS NULL THEN
        SELECT id INTO v_id FROM sut_version
        WHERE commit = p_commit
          AND branch IS NOT DISTINCT FROM p_branch
          AND build_config IS NOT DISTINCT FROM p_build_config
          AND tag IS NOT DISTINCT FROM p_tag;
    END IF;
    RETURN v_id;
END
$$;

CREATE FUNCTION ensure_environment(p_details jsonb) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    INSERT INTO environment (details) VALUES (p_details)
    ON CONFLICT (fingerprint) DO NOTHING
    RETURNING id INTO v_id;

    IF v_id IS NULL THEN
        SELECT id INTO v_id FROM environment WHERE fingerprint = jsonb_sha256(p_details);
    END IF;
    RETURN v_id;
END
$$;

CREATE FUNCTION ensure_client_device(p_name text) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    INSERT INTO client_device (name) VALUES (p_name)
    ON CONFLICT (name) DO NOTHING
    RETURNING id INTO v_id;

    IF v_id IS NULL THEN
        SELECT id INTO v_id FROM client_device WHERE name = p_name;
    END IF;
    RETURN v_id;
END
$$;

CREATE FUNCTION ensure_asset(p_name text, p_version text, p_content_hash text DEFAULT NULL) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    INSERT INTO asset (name, version, content_hash) VALUES (p_name, p_version, p_content_hash)
    ON CONFLICT (name, version, content_hash) DO NOTHING
    RETURNING id INTO v_id;

    IF v_id IS NULL THEN
        SELECT id INTO v_id FROM asset
        WHERE name = p_name
          AND version IS NOT DISTINCT FROM p_version
          AND content_hash IS NOT DISTINCT FROM p_content_hash;
    END IF;
    RETURN v_id;
END
$$;

-- Asset order and duplicates don't matter; parameter types do ({"n": 1} and
-- {"n": "1"} are different test cases), and so do parameter units (10 Mbit/s
-- and 10 kbit/s are different test cases).
CREATE FUNCTION test_case_fingerprint(
    p_scenario_version_id bigint, p_asset_ids bigint[], p_parameters jsonb,
    p_parameter_units jsonb DEFAULT '{}'
) RETURNS bytea
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    RETURN jsonb_sha256(jsonb_build_array(
        p_scenario_version_id,
        to_jsonb(ARRAY(SELECT DISTINCT a FROM unnest(coalesce(p_asset_ids, '{}')) AS a ORDER BY a)),
        coalesce(p_parameters, '{}'),
        coalesce(p_parameter_units, '{}')
    ));

CREATE FUNCTION ensure_test_case(
    p_scenario_version_id bigint, p_asset_ids bigint[] DEFAULT '{}', p_parameters jsonb DEFAULT '{}',
    p_parameter_units jsonb DEFAULT '{}'
) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_fingerprint bytea := test_case_fingerprint(
        p_scenario_version_id, p_asset_ids, p_parameters, p_parameter_units);
    v_id          bigint;
BEGIN
    INSERT INTO test_case (scenario_version_id, parameters, parameter_units, fingerprint)
    VALUES (p_scenario_version_id, coalesce(p_parameters, '{}'),
            coalesce(p_parameter_units, '{}'), v_fingerprint)
    ON CONFLICT (fingerprint) DO NOTHING
    RETURNING id INTO v_id;

    IF v_id IS NULL THEN
        SELECT id INTO v_id FROM test_case WHERE fingerprint = v_fingerprint;
        RETURN v_id;
    END IF;

    INSERT INTO test_case_asset (test_case_id, asset_id)
    SELECT DISTINCT v_id, a FROM unnest(coalesce(p_asset_ids, '{}')) AS a;
    RETURN v_id;
END
$$;
