-- Derived metrics are no longer versioned (FR-17, ADR 0017). A derived metric
-- has one definition, which can be changed. It is evaluated at query time, so
-- a change applies to every analysis, historical runs included, and to the
-- derived metrics built on it.
--
-- Existing data: the latest version of each derived metric becomes its
-- definition; older versions are dropped. A metric that pinned an older
-- version of another one uses that metric's definition from now on.

-- Without pinned versions, the latest versions may use each other in a circle
-- (A v2 uses B v1, B v2 uses A v1). That cannot be carried over.
DO $$
DECLARE
    v_names text;
BEGIN
    WITH RECURSIVE edge AS (
        SELECT l.derived_metric_id AS metric_id, d.uses_derived_metric_id AS uses_id
        FROM derived_metric_dependency d
        JOIN latest_derived_metric_version l
          ON l.derived_metric_version_id = d.derived_metric_version_id
    ), reach(metric_id, uses_id) AS (
        SELECT metric_id, uses_id FROM edge
        UNION
        SELECT r.metric_id, e.uses_id FROM reach r JOIN edge e ON e.metric_id = r.uses_id
    )
    SELECT string_agg(dm.name, ', ' ORDER BY dm.name) INTO v_names
    FROM derived_metric dm
    WHERE EXISTS (SELECT 1 FROM reach r WHERE r.metric_id = dm.id AND r.uses_id = dm.id);

    IF v_names IS NOT NULL THEN
        RAISE EXCEPTION 'the latest versions of these derived metrics use each other in a circle: %', v_names
            USING ERRCODE = 'TR007',
                  HINT = 'Save a new version of one of them that does not use the others, then migrate again.';
    END IF;
END
$$;

-- The definition moves from the latest version to the derived metric ---------

DROP TRIGGER immutable_rows ON derived_metric;

ALTER TABLE derived_metric
    ADD COLUMN expression  text CHECK (btrim(expression) <> ''),
    ADD COLUMN unit        text,
    ADD COLUMN description text NOT NULL DEFAULT '',
    ADD COLUMN updated_at  timestamptz NOT NULL DEFAULT now();

UPDATE derived_metric dm
SET expression = l.expression, unit = l.unit, description = l.description,
    updated_at = l.created_at
FROM latest_derived_metric_version l
WHERE l.derived_metric_id = dm.id;

ALTER TABLE derived_metric
    ALTER COLUMN expression SET NOT NULL,
    ALTER COLUMN unit SET NOT NULL;

CREATE TEMP TABLE derived_metric_use AS
SELECT l.derived_metric_id, d.uses_derived_metric_id
FROM derived_metric_dependency d
JOIN latest_derived_metric_version l
  ON l.derived_metric_version_id = d.derived_metric_version_id;

DROP VIEW latest_derived_metric_version;
DROP FUNCTION save_derived_metric(text, text, text, text, bigint[]);
DROP TABLE derived_metric_dependency;
DROP TABLE derived_metric_version;

-- Composition (FR-17): the derived metrics an expression uses. It follows the
-- expression and is replaced with it. save_derived_metric() keeps it free of
-- cycles (TR007).
CREATE TABLE derived_metric_dependency (
    derived_metric_id      bigint NOT NULL REFERENCES derived_metric,
    uses_derived_metric_id bigint NOT NULL REFERENCES derived_metric,
    PRIMARY KEY (derived_metric_id, uses_derived_metric_id),
    CHECK (uses_derived_metric_id <> derived_metric_id)
);

CREATE INDEX derived_metric_dependency_uses_idx
    ON derived_metric_dependency (uses_derived_metric_id);

INSERT INTO derived_metric_dependency (derived_metric_id, uses_derived_metric_id)
SELECT derived_metric_id, uses_derived_metric_id FROM derived_metric_use;

DROP TABLE derived_metric_use;

-- Guards ----------------------------------------------------------------------
-- The definition can change. The name cannot: expressions and saved views
-- refer to a derived metric by its name. Derived metrics are not deleted.

CREATE FUNCTION reject_derived_metric_rename() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'derived metric % cannot be renamed: expressions refer to it by name', OLD.name
        USING ERRCODE = 'TR001';
END
$$;

CREATE TRIGGER derived_metric_keep_name BEFORE UPDATE OF name ON derived_metric
    FOR EACH ROW WHEN (NEW.name IS DISTINCT FROM OLD.name)
    EXECUTE FUNCTION reject_derived_metric_rename();

CREATE TRIGGER derived_metric_touch BEFORE UPDATE ON derived_metric
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TRIGGER derived_metric_no_delete BEFORE DELETE OR TRUNCATE ON derived_metric
    FOR EACH STATEMENT EXECUTE FUNCTION enforce_immutable('never');

-- Saving ----------------------------------------------------------------------
-- Creates a derived metric or changes its definition. p_uses names the derived
-- metrics the expression uses; the backend takes them from the validated
-- expression. Returns the id and whether the metric is new. Saving the same
-- content again changes nothing.
--
-- Saves run one after the other, so that two of them cannot close a cycle
-- together. Reads are not blocked.
CREATE FUNCTION save_derived_metric(
    p_name text, p_expression text, p_unit text, p_description text DEFAULT '',
    p_uses text[] DEFAULT '{}',
    OUT metric_id bigint, OUT created boolean
)
    LANGUAGE plpgsql AS $$
DECLARE
    v_names   text[] := coalesce(p_uses, '{}');
    v_unknown text;
    v_uses    bigint[];
    v_id      bigint;
BEGIN
    LOCK TABLE derived_metric IN SHARE ROW EXCLUSIVE MODE;

    SELECT string_agg(DISTINCT u, ', ') INTO v_unknown
    FROM unnest(v_names) AS u
    WHERE NOT EXISTS (SELECT 1 FROM derived_metric dm WHERE dm.name = u);
    IF v_unknown IS NOT NULL THEN
        RAISE EXCEPTION 'derived metric % uses derived metrics that do not exist: %', p_name, v_unknown
            USING ERRCODE = 'foreign_key_violation';
    END IF;
    v_uses := ARRAY(SELECT dm.id FROM derived_metric dm WHERE dm.name = ANY (v_names));

    SELECT dm.id INTO v_id FROM derived_metric dm WHERE dm.name = p_name;

    -- No cycles: the metric must not be among the metrics it uses, directly
    -- or through what they use.
    IF EXISTS (
        WITH RECURSIVE used(id) AS (
            SELECT u FROM unnest(v_uses) AS u
            UNION
            SELECT d.uses_derived_metric_id
            FROM derived_metric_dependency d
            JOIN used ON used.id = d.derived_metric_id
        )
        SELECT 1 FROM used WHERE used.id = v_id
    ) THEN
        RAISE EXCEPTION 'derived metric % would depend on itself', p_name
            USING ERRCODE = 'TR007';
    END IF;

    IF v_id IS NULL THEN
        INSERT INTO derived_metric (name, expression, unit, description)
        VALUES (p_name, p_expression, p_unit, p_description)
        RETURNING id INTO v_id;
        created := true;
    ELSE
        UPDATE derived_metric dm
        SET expression = p_expression, unit = p_unit, description = p_description
        WHERE dm.id = v_id
          AND (dm.expression, dm.unit, dm.description)
              IS DISTINCT FROM (p_expression, p_unit, p_description);
        created := false;
    END IF;

    DELETE FROM derived_metric_dependency d
    WHERE d.derived_metric_id = v_id AND d.uses_derived_metric_id <> ALL (v_uses);
    INSERT INTO derived_metric_dependency (derived_metric_id, uses_derived_metric_id)
    SELECT v_id, u FROM unnest(v_uses) AS u
    ON CONFLICT DO NOTHING;

    metric_id := v_id;
END
$$;
