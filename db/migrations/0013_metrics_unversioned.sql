-- Metric definitions are no longer versioned (FR-7, ADR 0018). A metric has
-- one definition, which can be changed in the web UI, like a derived metric
-- (ADR 0017). Results refer to the metric, so a change applies to every
-- result, historical ones included: stored values are plain numbers and are
-- read in the unit the metric has now.
--
-- Uploads create metrics that do not exist yet. They never change one: for
-- an existing metric the direction and description they send are ignored,
-- and a different unit or value type is rejected, because the values sent
-- with it would be stored under the wrong unit.
--
-- Existing data: the latest version of each metric becomes its definition,
-- and the results of all its versions move to it. Results uploaded under a
-- version with another unit are read in the latest unit from now on.
--
-- Error codes:
--   TR009  an upload defines a metric with another unit or value type
--   TR010  metric has results (delete rejected)

-- One value and one series per sample and metric: results of two versions of
-- a metric on the same sample cannot be merged.
DO $$
DECLARE
    v_names text;
BEGIN
    SELECT string_agg(DISTINCT m.name, ', ' ORDER BY m.name) INTO v_names
    FROM (
        SELECT v.metric_id
        FROM measurement r JOIN metric_version v ON v.id = r.metric_version_id
        GROUP BY r.sample_id, v.metric_id HAVING count(*) > 1
        UNION
        SELECT v.metric_id
        FROM series r JOIN metric_version v ON v.id = r.metric_version_id
        GROUP BY r.sample_id, v.metric_id HAVING count(*) > 1
    ) d
    JOIN metric m ON m.id = d.metric_id;

    IF v_names IS NOT NULL THEN
        RAISE EXCEPTION 'samples have results of several versions of these metrics: %', v_names
            USING ERRCODE = 'unique_violation',
                  HINT = 'Delete the runs with such samples, then migrate again.';
    END IF;
END
$$;

-- The definition moves from the latest version to the metric ------------------

DROP TRIGGER immutable_rows ON metric;

-- A metric without a version has no definition and no results.
DELETE FROM metric m
WHERE NOT EXISTS (SELECT 1 FROM metric_version v WHERE v.metric_id = m.id);

ALTER TABLE metric
    ADD COLUMN unit        text,  -- '' for dimensionless
    ADD COLUMN value_type  metric_value_type,
    ADD COLUMN direction   metric_direction,
    ADD COLUMN description text NOT NULL DEFAULT '',
    ADD COLUMN updated_at  timestamptz NOT NULL DEFAULT now();

UPDATE metric m
SET unit = l.unit, value_type = l.value_type, direction = l.direction,
    description = l.description, updated_at = l.created_at
FROM (
    SELECT DISTINCT ON (metric_id) metric_id, unit, value_type, direction, description, created_at
    FROM metric_version
    ORDER BY metric_id, version DESC
) l
WHERE l.metric_id = m.id;

ALTER TABLE metric
    ALTER COLUMN unit SET NOT NULL,
    ALTER COLUMN value_type SET NOT NULL,
    ALTER COLUMN direction SET NOT NULL;

CREATE TRIGGER metric_touch BEFORE UPDATE ON metric
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- Results refer to the metric ---------------------------------------------------
-- The column is rewritten in place. The values, and every other column, stay
-- as they are; the immutability guard stays on the tables.

CREATE FUNCTION pg_temp.metric_of_version(p_version_id bigint) RETURNS bigint
    LANGUAGE sql STABLE
    AS $$ SELECT metric_id FROM metric_version WHERE id = p_version_id $$;

ALTER TABLE measurement DROP CONSTRAINT measurement_metric_version_id_fkey;
ALTER TABLE measurement
    ALTER COLUMN metric_version_id TYPE bigint USING pg_temp.metric_of_version(metric_version_id);
ALTER TABLE measurement RENAME COLUMN metric_version_id TO metric_id;
ALTER TABLE measurement
    ADD CONSTRAINT measurement_metric_id_fkey FOREIGN KEY (metric_id) REFERENCES metric;
ALTER INDEX measurement_metric_version_idx RENAME TO measurement_metric_idx;

ALTER TABLE series DROP CONSTRAINT series_metric_version_id_fkey;
ALTER TABLE series
    ALTER COLUMN metric_version_id TYPE bigint USING pg_temp.metric_of_version(metric_version_id);
ALTER TABLE series RENAME COLUMN metric_version_id TO metric_id;
ALTER TABLE series
    ADD CONSTRAINT series_metric_id_fkey FOREIGN KEY (metric_id) REFERENCES metric;
ALTER TABLE series
    RENAME CONSTRAINT series_sample_id_metric_version_id_key TO series_sample_id_metric_id_key;
ALTER INDEX series_metric_version_idx RENAME TO series_metric_idx;

DROP FUNCTION pg_temp.metric_of_version(bigint);
DROP FUNCTION ensure_metric_version(text, text, metric_value_type, metric_direction, text);
DROP TABLE metric_version;
DROP FUNCTION metric_content_hash(text, metric_value_type, metric_direction, text);

-- Uploads -----------------------------------------------------------------------
-- Returns the id of the metric called p_name and creates it with the given
-- definition if there is none. An existing metric is not changed: its
-- direction and description stay, and another unit or value type raises
-- TR009.
--
-- The metric stays locked against deletion and renaming until the end of
-- the transaction, so the results that follow can refer to it.
CREATE FUNCTION ensure_metric(
    p_name text, p_unit text, p_value_type metric_value_type,
    p_direction metric_direction, p_description text DEFAULT ''
) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_metric metric;
BEGIN
    LOOP
        INSERT INTO metric (name, unit, value_type, direction, description)
        VALUES (p_name, p_unit, p_value_type, p_direction, p_description)
        ON CONFLICT (name) DO NOTHING
        RETURNING * INTO v_metric;
        IF FOUND THEN
            RETURN v_metric.id;
        END IF;

        -- Not found only if it was renamed or deleted in between.
        SELECT * INTO v_metric FROM metric WHERE name = p_name FOR KEY SHARE;
        EXIT WHEN FOUND;
    END LOOP;

    IF (v_metric.unit, v_metric.value_type) IS DISTINCT FROM (p_unit, p_value_type) THEN
        RAISE EXCEPTION 'metric % has unit "%" and value type %, the upload sends unit "%" and value type %',
            p_name, v_metric.unit, v_metric.value_type, p_unit, p_value_type
            USING ERRCODE = 'TR009',
                  HINT = 'Send the unit and value type of the metric, or change the metric in the web UI.';
    END IF;
    RETURN v_metric.id;
END
$$;

-- Editing (web UI only) -------------------------------------------------------------
-- Changes the definition of a metric. Returns its id, or NULL if there is
-- none called p_name. Saving the same content again changes nothing.
CREATE FUNCTION save_metric(
    p_name text, p_unit text, p_value_type metric_value_type,
    p_direction metric_direction, p_description text DEFAULT ''
) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    SELECT id INTO v_id FROM metric WHERE name = p_name FOR NO KEY UPDATE;
    IF v_id IS NULL THEN
        RETURN NULL;
    END IF;

    UPDATE metric m
    SET unit = p_unit, value_type = p_value_type, direction = p_direction,
        description = p_description
    WHERE m.id = v_id
      AND (m.unit, m.value_type, m.direction, m.description)
          IS DISTINCT FROM (p_unit, p_value_type, p_direction, p_description);
    RETURN v_id;
END
$$;

-- Renaming and deleting take the lock that saves of derived metrics take, so
-- the expressions that use the metric cannot change until the commit. The
-- backend takes it first, reads those expressions and rewrites them (rename)
-- or refuses (delete) in the same transaction.

-- Renames a metric. Returns its id, or NULL if there is none called p_name.
-- A taken name is a unique violation (23505).
CREATE FUNCTION rename_metric(p_name text, p_new_name text) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    LOCK TABLE derived_metric IN SHARE ROW EXCLUSIVE MODE;
    UPDATE metric SET name = p_new_name
    WHERE name = p_name
    RETURNING id INTO v_id;
    RETURN v_id;
END
$$;

-- Deletes a metric that has no results. Returns false if there is none
-- called p_name. Raises TR010 if measurements or series refer to it: results
-- are deleted only with their run (FR-8, FR-10).
CREATE FUNCTION delete_metric(p_name text) RETURNS boolean
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    LOCK TABLE derived_metric IN SHARE ROW EXCLUSIVE MODE;
    -- Waits for uploads that are using the metric.
    SELECT id INTO v_id FROM metric WHERE name = p_name FOR UPDATE;
    IF v_id IS NULL THEN
        RETURN false;
    END IF;

    IF EXISTS (SELECT 1 FROM measurement WHERE metric_id = v_id)
       OR EXISTS (SELECT 1 FROM series WHERE metric_id = v_id) THEN
        RAISE EXCEPTION 'metric % has results and cannot be deleted', p_name
            USING ERRCODE = 'TR010';
    END IF;

    DELETE FROM metric WHERE id = v_id;
    RETURN true;
END
$$;
