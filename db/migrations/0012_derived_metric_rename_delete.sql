-- Derived metrics can be renamed and deleted (FR-17, ADR 0017).
--
-- Both only through the functions below, which take the same lock as
-- save_derived_metric(), so they never run at the same time as a save.
-- Renaming changes the name only; the backend rewrites the expressions of
-- the derived metrics that use the renamed one in the same transaction.
-- A derived metric that other derived metrics use cannot be deleted.
--
-- Error code:
--   TR008  derived metric is used by other derived metrics

DROP TRIGGER derived_metric_keep_name ON derived_metric;
DROP FUNCTION reject_derived_metric_rename();
DROP TRIGGER derived_metric_no_delete ON derived_metric;

-- Deleting a derived metric removes the record of what it uses. What uses
-- it keeps it: the foreign key on uses_derived_metric_id stays restrictive.
ALTER TABLE derived_metric_dependency
    DROP CONSTRAINT derived_metric_dependency_derived_metric_id_fkey,
    ADD CONSTRAINT derived_metric_dependency_derived_metric_id_fkey
        FOREIGN KEY (derived_metric_id) REFERENCES derived_metric ON DELETE CASCADE;

-- Renames a derived metric. Returns its id, or NULL if there is none called
-- p_name. A taken name is a unique violation (23505).
CREATE FUNCTION rename_derived_metric(p_name text, p_new_name text) RETURNS bigint
    LANGUAGE plpgsql AS $$
DECLARE
    v_id bigint;
BEGIN
    LOCK TABLE derived_metric IN SHARE ROW EXCLUSIVE MODE;
    UPDATE derived_metric SET name = p_new_name
    WHERE name = p_name
    RETURNING id INTO v_id;
    RETURN v_id;
END
$$;

-- Deletes a derived metric. Returns false if there is none called p_name.
-- Raises TR008 if other derived metrics use it.
CREATE FUNCTION delete_derived_metric(p_name text) RETURNS boolean
    LANGUAGE plpgsql AS $$
DECLARE
    v_id    bigint;
    v_users text;
BEGIN
    LOCK TABLE derived_metric IN SHARE ROW EXCLUSIVE MODE;
    SELECT id INTO v_id FROM derived_metric WHERE name = p_name;
    IF v_id IS NULL THEN
        RETURN false;
    END IF;

    SELECT string_agg(m.name, ', ' ORDER BY m.name) INTO v_users
    FROM derived_metric_dependency d
    JOIN derived_metric m ON m.id = d.derived_metric_id
    WHERE d.uses_derived_metric_id = v_id;
    IF v_users IS NOT NULL THEN
        RAISE EXCEPTION 'derived metric % is used by %; change or delete those first', p_name, v_users
            USING ERRCODE = 'TR008';
    END IF;

    DELETE FROM derived_metric WHERE id = v_id;
    RETURN true;
END
$$;
