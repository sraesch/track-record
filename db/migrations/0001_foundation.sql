-- Shared types and helper functions used by all later migrations.

CREATE TYPE run_status AS ENUM ('running', 'completed', 'failed', 'aborted');
CREATE TYPE run_trigger AS ENUM ('nightly', 'manual', 'ci');
CREATE TYPE metric_value_type AS ENUM ('float', 'integer', 'boolean');
CREATE TYPE metric_direction AS ENUM ('higher_is_better', 'lower_is_better');
CREATE TYPE token_scope AS ENUM ('read', 'annotate', 'write');

-- SHA-256 of a JSON value. jsonb normalizes key order and whitespace, so equal
-- objects hash the same. Declared IMMUTABLE so it can back generated columns;
-- the database encoding (UTF8) never changes.
CREATE FUNCTION jsonb_sha256(p_value jsonb) RETURNS bytea
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    RETURN sha256(convert_to(p_value::text, 'UTF8'));

-- True if the value is a JSON object whose values are all numbers, strings or
-- booleans. Parameters keep their JSON type but may not nest.
CREATE FUNCTION is_flat_scalar_object(p_value jsonb) RETURNS boolean
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    RETURN jsonb_typeof(p_value) = 'object'
        AND NOT EXISTS (
            SELECT 1 FROM jsonb_each(p_value)
            WHERE jsonb_typeof(value) NOT IN ('number', 'string', 'boolean')
        );

-- True if the value maps parameter names to unit symbols (FR-13a): a JSON
-- object of strings whose keys are all numeric parameters. Parameters without
-- a unit are simply left out.
CREATE FUNCTION are_parameter_units(p_units jsonb, p_parameters jsonb) RETURNS boolean
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    RETURN jsonb_typeof(p_units) = 'object'
        AND NOT EXISTS (
            SELECT 1 FROM jsonb_each(p_units) AS u
            WHERE jsonb_typeof(u.value) <> 'string'
               OR jsonb_typeof(p_parameters -> u.key) IS DISTINCT FROM 'number'
        );

-- Immutability guard (FR-6, FR-8), installed as statement-level trigger.
-- UPDATE and TRUNCATE are always rejected. DELETE is rejected unless the
-- trigger argument is 'deletable_with_run' and delete_run() is running.
--
-- Error codes:
--   TR001  row is immutable
--   TR002  delete outside delete_run()
CREATE FUNCTION enforce_immutable() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP IN ('UPDATE', 'TRUNCATE') THEN
        RAISE EXCEPTION '% on % is not allowed: rows are immutable', TG_OP, TG_TABLE_NAME
            USING ERRCODE = 'TR001';
    END IF;

    IF TG_ARGV[0] = 'deletable_with_run'
       AND current_setting('track_record.deleting_run', true) = 'on' THEN
        RETURN NULL;
    END IF;

    RAISE EXCEPTION 'DELETE on % is only allowed through delete_run()', TG_TABLE_NAME
        USING ERRCODE = 'TR002';
END
$$;

-- Installs the immutability guard on a table.
CREATE FUNCTION make_immutable(p_table regclass, p_deletable_with_run boolean) RETURNS void
    LANGUAGE plpgsql AS $$
BEGIN
    EXECUTE format(
        'CREATE TRIGGER immutable_rows BEFORE UPDATE OR DELETE OR TRUNCATE ON %s '
        'FOR EACH STATEMENT EXECUTE FUNCTION enforce_immutable(%L)',
        p_table,
        CASE WHEN p_deletable_with_run THEN 'deletable_with_run' ELSE 'never' END
    );
END
$$;

-- Keeps updated_at current on mutable tables.
CREATE FUNCTION touch_updated_at() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END
$$;
