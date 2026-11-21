-- Versioned definitions (FR-7): suites, scenarios, metrics, derived metrics
-- and aliases. Each has a name table and an immutable version table.
--
-- Definitions sent by the harness (suites, scenarios, metrics) are deduplicated
-- by content: sending the same content again returns the existing version, so
-- uploads stay idempotent (FR-3). Definitions edited in the UI (derived
-- metrics, aliases) get a new version unless the content equals the latest one.

-- Suites ---------------------------------------------------------------------

CREATE TABLE suite (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text NOT NULL UNIQUE CHECK (btrim(name) <> ''),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE suite_version (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    suite_id     bigint NOT NULL REFERENCES suite,
    version      integer NOT NULL CHECK (version > 0),
    definition   jsonb NOT NULL,
    content_hash bytea NOT NULL GENERATED ALWAYS AS (jsonb_sha256(definition)) STORED,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (suite_id, version),
    UNIQUE (suite_id, content_hash)
);

-- Scenarios ------------------------------------------------------------------
-- A changed definition is a new scenario version and therefore a different
-- test case: trends never mix versions.

CREATE TABLE scenario (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text NOT NULL UNIQUE CHECK (btrim(name) <> ''),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE scenario_version (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scenario_id  bigint NOT NULL REFERENCES scenario,
    version      integer NOT NULL CHECK (version > 0),
    definition   jsonb NOT NULL,
    content_hash bytea NOT NULL GENERATED ALWAYS AS (jsonb_sha256(definition)) STORED,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scenario_id, version),
    UNIQUE (scenario_id, content_hash)
);

-- Metrics --------------------------------------------------------------------

CREATE FUNCTION metric_content_hash(
    p_unit text, p_value_type metric_value_type, p_direction metric_direction, p_description text
) RETURNS bytea
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    RETURN jsonb_sha256(jsonb_build_array(p_unit, p_value_type::text, p_direction::text, p_description));

CREATE TABLE metric (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text NOT NULL UNIQUE CHECK (btrim(name) <> ''),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE metric_version (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    metric_id    bigint NOT NULL REFERENCES metric,
    version      integer NOT NULL CHECK (version > 0),
    unit         text NOT NULL,  -- '' for dimensionless
    value_type   metric_value_type NOT NULL,
    direction    metric_direction NOT NULL,
    description  text NOT NULL DEFAULT '',
    content_hash bytea NOT NULL GENERATED ALWAYS AS
                     (metric_content_hash(unit, value_type, direction, description)) STORED,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (metric_id, version),
    UNIQUE (metric_id, content_hash)
);

-- Derived metrics (FR-17, FR-17a) ---------------------------------------------
-- The expression is validated by the backend before it is saved.

CREATE TABLE derived_metric (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text NOT NULL UNIQUE CHECK (btrim(name) <> ''),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE derived_metric_version (
    id                bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    derived_metric_id bigint NOT NULL REFERENCES derived_metric,
    version           integer NOT NULL CHECK (version > 0),
    expression        text NOT NULL CHECK (btrim(expression) <> ''),
    unit              text NOT NULL,
    description       text NOT NULL DEFAULT '',
    created_at        timestamptz NOT NULL DEFAULT now(),
    UNIQUE (derived_metric_id, version),
    UNIQUE (id, derived_metric_id)
);

-- Composition (FR-17): a derived metric can use other derived metrics. Each
-- version pins the exact versions it uses, so a new version of a used metric
-- only takes effect through a new version of the metric built on it.
--
-- A version can only use versions that already existed when it was saved
-- (smaller id), and at most one version of each derived metric. Together with
-- the check in save_derived_metric() this rules out cycles.
--
-- Error code:
--   TR007  derived metrics would form a cycle
CREATE TABLE derived_metric_dependency (
    derived_metric_version_id bigint NOT NULL REFERENCES derived_metric_version,
    uses_version_id           bigint NOT NULL,
    uses_derived_metric_id    bigint NOT NULL,
    PRIMARY KEY (derived_metric_version_id, uses_version_id),
    UNIQUE (derived_metric_version_id, uses_derived_metric_id),
    FOREIGN KEY (uses_version_id, uses_derived_metric_id)
        REFERENCES derived_metric_version (id, derived_metric_id),
    CHECK (uses_version_id < derived_metric_version_id)
);

CREATE INDEX derived_metric_dependency_uses_idx ON derived_metric_dependency (uses_version_id);

CREATE VIEW latest_derived_metric_version AS
SELECT DISTINCT ON (dm.id)
       dm.id AS derived_metric_id, dm.name, v.id AS derived_metric_version_id,
       v.version, v.expression, v.unit, v.description, v.created_at
FROM derived_metric dm
JOIN derived_metric_version v ON v.derived_metric_id = dm.id
ORDER BY dm.id, v.version DESC;

-- Aliases (FR-14) ------------------------------------------------------------
-- Maps a value of a dimension to another value. The dimension is e.g.
-- 'client_device' or one case parameter ('parameter:solution'). Values are
-- JSON scalars so that parameter values keep their type: 10 and "10" are
-- different values. A device name is a JSON string.
-- A version with target_value NULL removes the alias.

CREATE TABLE alias (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    dimension    text NOT NULL CHECK (btrim(dimension) <> ''),
    source_value jsonb NOT NULL
                     CHECK (jsonb_typeof(source_value) IN ('number', 'string', 'boolean')),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (dimension, source_value)
);

CREATE TABLE alias_version (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    alias_id     bigint NOT NULL REFERENCES alias,
    version      integer NOT NULL CHECK (version > 0),
    target_value jsonb CHECK (jsonb_typeof(target_value) IN ('number', 'string', 'boolean')),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (alias_id, version)
);

CREATE VIEW current_alias AS
SELECT a.id AS alias_id, a.dimension, a.source_value, v.target_value, v.version
FROM alias a
JOIN LATERAL (
    SELECT target_value, version FROM alias_version
    WHERE alias_id = a.id ORDER BY version DESC LIMIT 1
) v ON true
WHERE v.target_value IS NOT NULL;

-- Immutability ---------------------------------------------------------------

SELECT make_immutable(t, false)
FROM unnest(ARRAY[
    'suite', 'suite_version', 'scenario', 'scenario_version', 'metric', 'metric_version',
    'derived_metric', 'derived_metric_version', 'derived_metric_dependency',
    'alias', 'alias_version'
]::regclass[]) AS t;

-- Functions ------------------------------------------------------------------
-- The ensure_* functions return the id of the matching version, creating it if
-- needed. Locking the name row serializes concurrent version assignment.

CREATE FUNCTION ensure_suite_version(p_name text, p_definition jsonb) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_parent bigint;
    v_id     bigint;
BEGIN
    INSERT INTO suite (name) VALUES (p_name) ON CONFLICT (name) DO NOTHING;
    SELECT id INTO v_parent FROM suite WHERE name = p_name FOR UPDATE;

    SELECT id INTO v_id FROM suite_version
    WHERE suite_id = v_parent AND content_hash = jsonb_sha256(p_definition);

    IF v_id IS NULL THEN
        INSERT INTO suite_version (suite_id, version, definition)
        SELECT v_parent, coalesce(max(version), 0) + 1, p_definition
        FROM suite_version WHERE suite_id = v_parent
        RETURNING id INTO v_id;
    END IF;
    RETURN v_id;
END
$$;

CREATE FUNCTION ensure_scenario_version(p_name text, p_definition jsonb) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_parent bigint;
    v_id     bigint;
BEGIN
    INSERT INTO scenario (name) VALUES (p_name) ON CONFLICT (name) DO NOTHING;
    SELECT id INTO v_parent FROM scenario WHERE name = p_name FOR UPDATE;

    SELECT id INTO v_id FROM scenario_version
    WHERE scenario_id = v_parent AND content_hash = jsonb_sha256(p_definition);

    IF v_id IS NULL THEN
        INSERT INTO scenario_version (scenario_id, version, definition)
        SELECT v_parent, coalesce(max(version), 0) + 1, p_definition
        FROM scenario_version WHERE scenario_id = v_parent
        RETURNING id INTO v_id;
    END IF;
    RETURN v_id;
END
$$;

CREATE FUNCTION ensure_metric_version(
    p_name text, p_unit text, p_value_type metric_value_type,
    p_direction metric_direction, p_description text DEFAULT ''
) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_parent bigint;
    v_id     bigint;
BEGIN
    INSERT INTO metric (name) VALUES (p_name) ON CONFLICT (name) DO NOTHING;
    SELECT id INTO v_parent FROM metric WHERE name = p_name FOR UPDATE;

    SELECT id INTO v_id FROM metric_version
    WHERE metric_id = v_parent
      AND content_hash = metric_content_hash(p_unit, p_value_type, p_direction, p_description);

    IF v_id IS NULL THEN
        INSERT INTO metric_version (metric_id, version, unit, value_type, direction, description)
        SELECT v_parent, coalesce(max(version), 0) + 1, p_unit, p_value_type, p_direction, p_description
        FROM metric_version WHERE metric_id = v_parent
        RETURNING id INTO v_id;
    END IF;
    RETURN v_id;
END
$$;

-- p_uses lists the derived metric versions the expression builds on. The
-- backend takes them from the validated expression.
CREATE FUNCTION save_derived_metric(
    p_name text, p_expression text, p_unit text, p_description text DEFAULT '',
    p_uses bigint[] DEFAULT '{}'
) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_parent bigint;
    v_latest derived_metric_version;
    v_uses   bigint[] := ARRAY(SELECT DISTINCT u FROM unnest(coalesce(p_uses, '{}')) AS u ORDER BY u);
    v_id     bigint;
BEGIN
    INSERT INTO derived_metric (name) VALUES (p_name) ON CONFLICT (name) DO NOTHING;
    SELECT id INTO v_parent FROM derived_metric WHERE name = p_name FOR UPDATE;

    IF EXISTS (
        SELECT 1 FROM unnest(v_uses) AS u
        WHERE NOT EXISTS (SELECT 1 FROM derived_metric_version WHERE id = u)
    ) THEN
        RAISE EXCEPTION 'derived metric % uses a version that does not exist', p_name
            USING ERRCODE = 'foreign_key_violation';
    END IF;

    -- No cycles: the metric must not appear among the versions it uses,
    -- directly or through their own dependencies.
    IF EXISTS (
        WITH RECURSIVE used(version_id) AS (
            SELECT u FROM unnest(v_uses) AS u
            UNION
            SELECT d.uses_version_id
            FROM derived_metric_dependency d
            JOIN used ON used.version_id = d.derived_metric_version_id
        )
        SELECT 1 FROM used
        JOIN derived_metric_version v ON v.id = used.version_id
        WHERE v.derived_metric_id = v_parent
    ) THEN
        RAISE EXCEPTION 'derived metric % would depend on itself', p_name
            USING ERRCODE = 'TR007';
    END IF;

    SELECT * INTO v_latest FROM derived_metric_version
    WHERE derived_metric_id = v_parent ORDER BY version DESC LIMIT 1;

    IF v_latest.id IS NOT NULL
       AND (v_latest.expression, v_latest.unit, v_latest.description)
           = (p_expression, p_unit, p_description)
       AND v_uses = ARRAY(SELECT uses_version_id FROM derived_metric_dependency
                          WHERE derived_metric_version_id = v_latest.id
                          ORDER BY uses_version_id) THEN
        RETURN v_latest.id;
    END IF;

    INSERT INTO derived_metric_version (derived_metric_id, version, expression, unit, description)
    VALUES (v_parent, coalesce(v_latest.version, 0) + 1, p_expression, p_unit, p_description)
    RETURNING id INTO v_id;

    INSERT INTO derived_metric_dependency
        (derived_metric_version_id, uses_version_id, uses_derived_metric_id)
    SELECT v_id, v.id, v.derived_metric_id
    FROM derived_metric_version v
    WHERE v.id = ANY (v_uses);
    RETURN v_id;
END
$$;

-- Sets (or with p_target NULL, removes) an alias. Returns the alias version id.
-- Values are JSON scalars, e.g. to_jsonb('pixel-8-a'::text) or '10'::jsonb.
CREATE FUNCTION set_alias(p_dimension text, p_source jsonb, p_target jsonb) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_parent bigint;
    v_latest alias_version;
    v_id     bigint;
BEGIN
    IF p_source = p_target THEN
        RAISE EXCEPTION 'alias source and target must differ' USING ERRCODE = 'check_violation';
    END IF;

    -- Removing an alias that never existed is a no-op.
    IF p_target IS NULL AND NOT EXISTS (
        SELECT 1 FROM alias WHERE dimension = p_dimension AND source_value = p_source
    ) THEN
        RETURN NULL;
    END IF;

    INSERT INTO alias (dimension, source_value) VALUES (p_dimension, p_source)
    ON CONFLICT (dimension, source_value) DO NOTHING;
    SELECT id INTO v_parent FROM alias
    WHERE dimension = p_dimension AND source_value = p_source FOR UPDATE;

    SELECT * INTO v_latest FROM alias_version
    WHERE alias_id = v_parent ORDER BY version DESC LIMIT 1;

    IF v_latest.id IS NOT NULL AND v_latest.target_value IS NOT DISTINCT FROM p_target THEN
        RETURN v_latest.id;
    END IF;

    INSERT INTO alias_version (alias_id, version, target_value)
    VALUES (v_parent, coalesce(v_latest.version, 0) + 1, p_target)
    RETURNING id INTO v_id;
    RETURN v_id;
END
$$;
