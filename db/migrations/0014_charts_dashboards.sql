-- Charts and dashboards with their history (FR-26, ADR 0022).
--
-- A chart or dashboard is a row without content of its own; its definitions
-- are revisions. Every save adds a revision, the latest one is the current
-- definition, and revisions never change. Restoring saves the content of an
-- old revision as a new one, so the history only grows. Revisions are
-- removed only together with their chart or dashboard.
--
-- A save names the revision it is based on and is rejected if that is no
-- longer the latest: the app has no user identity, and without this check
-- the last of two saves would silently win.
--
-- Which charts a dashboard uses is recorded for its current revision in
-- dashboard_chart, so that a chart in use cannot be deleted. Built-in charts
-- are not rows: a dashboard refers to them by name. Which metrics a chart
-- uses is read from its spec by the backend and not recorded here.
--
-- Writes go through save_chart(), delete_chart(), save_dashboard() and
-- delete_dashboard().
--
-- Error codes:
--   TR011  the revision a save is based on is no longer the latest
--   TR012  chart is used by the current revision of a dashboard (delete rejected)

-- Immutable revisions ----------------------------------------------------------
-- UPDATE and TRUNCATE are always rejected (TR001). DELETE only while
-- delete_<argument>() runs (TR002), which removes a chart or dashboard with
-- its revisions.
CREATE FUNCTION enforce_revision_immutable() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP IN ('UPDATE', 'TRUNCATE') THEN
        RAISE EXCEPTION '% on % is not allowed: revisions are immutable', TG_OP, TG_TABLE_NAME
            USING ERRCODE = 'TR001';
    END IF;

    IF current_setting('track_record.deleting_' || TG_ARGV[0], true) = 'on' THEN
        RETURN NULL;
    END IF;

    RAISE EXCEPTION 'DELETE on % is only allowed through delete_%()', TG_TABLE_NAME, TG_ARGV[0]
        USING ERRCODE = 'TR002';
END
$$;

-- Charts -----------------------------------------------------------------------

CREATE TABLE chart (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- The spec is the JSON of ADR 0022 ("Spec"). The backend checks that it is
-- well-formed before it saves it; here only its outline is checked.
CREATE TABLE chart_revision (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    chart_id    bigint NOT NULL REFERENCES chart ON DELETE CASCADE,
    revision    integer NOT NULL CHECK (revision > 0),
    title       text NOT NULL CHECK (btrim(title) <> ''),
    description text NOT NULL DEFAULT '',
    spec        jsonb NOT NULL CHECK (jsonb_typeof(spec) = 'object' AND spec ? 'format'),
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (chart_id, revision)
);

CREATE TRIGGER immutable_rows BEFORE UPDATE OR DELETE OR TRUNCATE ON chart_revision
    FOR EACH STATEMENT EXECUTE FUNCTION enforce_revision_immutable('chart');

CREATE VIEW current_chart_revision AS
SELECT DISTINCT ON (r.chart_id) r.*
FROM chart_revision r
ORDER BY r.chart_id, r.revision DESC;

-- Dashboards -------------------------------------------------------------------

-- A placement puts a chart on a dashboard (ADR 0022, "Storage and history"):
--   {"chart": {"id": 12} | {"builtin": "trends"}, "layout": {...}, "variables": {...}}
-- Only the reference to the chart is checked here: an id is a whole number
-- in the range of bigint. The backend checks the rest, including the names
-- of built-in charts.
CREATE FUNCTION valid_dashboard_placements(p_placements jsonb) RETURNS boolean
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    RETURN jsonb_typeof(p_placements) = 'array'
        AND NOT EXISTS (
            SELECT 1 FROM jsonb_array_elements(p_placements) AS p
            WHERE (
                jsonb_typeof(p) = 'object'
                AND jsonb_typeof(p -> 'chart') = 'object'
                AND CASE
                    WHEN p -> 'chart' ? 'id' THEN
                        NOT (p -> 'chart' ? 'builtin')
                        AND CASE
                            WHEN jsonb_typeof(p -> 'chart' -> 'id') = 'number' THEN
                                (p -> 'chart' -> 'id')::numeric = trunc((p -> 'chart' -> 'id')::numeric)
                                AND (p -> 'chart' -> 'id')::numeric BETWEEN 1 AND 9223372036854775807
                            ELSE false
                        END
                    WHEN p -> 'chart' ? 'builtin' THEN
                        jsonb_typeof(p -> 'chart' -> 'builtin') = 'string'
                        AND btrim(p -> 'chart' ->> 'builtin') <> ''
                    ELSE false
                END
            ) IS NOT TRUE
        );

CREATE TABLE dashboard (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE dashboard_revision (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    dashboard_id bigint NOT NULL REFERENCES dashboard ON DELETE CASCADE,
    revision     integer NOT NULL CHECK (revision > 0),
    title        text NOT NULL CHECK (btrim(title) <> ''),
    description  text NOT NULL DEFAULT '',
    variables    jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(variables) = 'array'),
    placements   jsonb NOT NULL DEFAULT '[]' CHECK (valid_dashboard_placements(placements)),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (dashboard_id, revision)
);

CREATE TRIGGER immutable_rows BEFORE UPDATE OR DELETE OR TRUNCATE ON dashboard_revision
    FOR EACH STATEMENT EXECUTE FUNCTION enforce_revision_immutable('dashboard');

CREATE VIEW current_dashboard_revision AS
SELECT DISTINCT ON (r.dashboard_id) r.*
FROM dashboard_revision r
ORDER BY r.dashboard_id, r.revision DESC;

-- The saved charts that the current revision of a dashboard places. It
-- follows the placements and is replaced with them. The foreign key on
-- chart_id keeps a chart in use from being deleted, also outside
-- delete_chart().
CREATE TABLE dashboard_chart (
    dashboard_id bigint NOT NULL REFERENCES dashboard ON DELETE CASCADE,
    chart_id     bigint NOT NULL REFERENCES chart,
    PRIMARY KEY (dashboard_id, chart_id)
);

CREATE INDEX dashboard_chart_chart_idx ON dashboard_chart (chart_id);

-- Stale saves -------------------------------------------------------------------
-- A save is based on a revision that is no longer the latest (TR011).
--
-- save_chart() and save_dashboard() lock the chart or dashboard, then compare
-- p_based_on with the latest revision. Under REPEATABLE READ, a save that
-- waited for the lock still reads the revisions of its snapshot and misses the
-- one the other save added; its insert then collides with that revision on
-- (…_id, revision), the only unique key it can hit. Both cases raise this.
CREATE FUNCTION raise_stale_save(p_kind text, p_id bigint, p_based_on integer) RETURNS void
    LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% % was changed: the save is based on revision %, which is no longer the latest',
            p_kind, p_id, coalesce(p_based_on::text, 'none')
        USING ERRCODE = 'TR011',
              HINT = 'Load the latest revision and apply the change to it.';
END
$$;

-- Saving charts ----------------------------------------------------------------
-- Creates a chart (p_chart_id and p_based_on NULL) or adds a revision to one.
-- p_based_on is the revision the save started from; if it is no longer the
-- latest, the save is rejected (TR011). Returns the chart, the number of the
-- new revision and its id; all NULL if there is no chart p_chart_id.
--
-- Saves of the same chart run one after the other (row lock on the chart);
-- see raise_stale_save() for REPEATABLE READ.
CREATE FUNCTION save_chart(
    p_chart_id bigint, p_based_on integer,
    p_title text, p_description text, p_spec jsonb,
    OUT chart_id bigint, OUT revision integer, OUT revision_id bigint
)
    LANGUAGE plpgsql AS $$
DECLARE
    v_latest integer;
BEGIN
    IF p_chart_id IS NULL THEN
        IF p_based_on IS NOT NULL THEN
            RAISE EXCEPTION 'a new chart is not based on a revision'
                USING ERRCODE = 'invalid_parameter_value';
        END IF;
        INSERT INTO chart DEFAULT VALUES RETURNING chart.id INTO chart_id;
        revision := 1;
    ELSE
        PERFORM FROM chart c WHERE c.id = p_chart_id FOR UPDATE;
        IF NOT FOUND THEN
            RETURN;
        END IF;

        SELECT max(r.revision) INTO v_latest
        FROM chart_revision r WHERE r.chart_id = p_chart_id;
        IF p_based_on IS DISTINCT FROM v_latest THEN
            PERFORM raise_stale_save('chart', p_chart_id, p_based_on);
        END IF;
        chart_id := p_chart_id;
        revision := v_latest + 1;
    END IF;

    BEGIN
        INSERT INTO chart_revision AS r (chart_id, revision, title, description, spec)
        VALUES (save_chart.chart_id, save_chart.revision, p_title, coalesce(p_description, ''), p_spec)
        RETURNING r.id INTO revision_id;
    EXCEPTION WHEN unique_violation THEN
        PERFORM raise_stale_save('chart', save_chart.chart_id, p_based_on);
    END;
END
$$;

-- Deletes a chart with its revisions. Returns false if there is none.
-- Raises TR012 if the current revision of a dashboard places it.
CREATE FUNCTION delete_chart(p_chart_id bigint) RETURNS boolean
    LANGUAGE plpgsql AS $$
DECLARE
    v_users text;
BEGIN
    -- The lock also waits for a dashboard save that is placing the chart.
    PERFORM FROM chart c WHERE c.id = p_chart_id FOR UPDATE;
    IF NOT FOUND THEN
        RETURN false;
    END IF;

    SELECT string_agg(format('%s (%s)', d.title, d.dashboard_id), ', ' ORDER BY d.dashboard_id)
        INTO v_users
    FROM dashboard_chart u
    JOIN current_dashboard_revision d ON d.dashboard_id = u.dashboard_id
    WHERE u.chart_id = p_chart_id;
    IF v_users IS NOT NULL THEN
        RAISE EXCEPTION 'chart % is used by the dashboards %; remove it from them first', p_chart_id, v_users
            USING ERRCODE = 'TR012';
    END IF;

    PERFORM set_config('track_record.deleting_chart', 'on', true);
    DELETE FROM chart c WHERE c.id = p_chart_id;
    PERFORM set_config('track_record.deleting_chart', 'off', true);
    RETURN true;
END
$$;

-- Saving dashboards ------------------------------------------------------------
-- Like save_chart(), with the variables and placements of the dashboard.
-- The saved charts it places must exist (23503); dashboard_chart is set to
-- them.
CREATE FUNCTION save_dashboard(
    p_dashboard_id bigint, p_based_on integer,
    p_title text, p_description text, p_variables jsonb, p_placements jsonb,
    OUT dashboard_id bigint, OUT revision integer, OUT revision_id bigint
)
    LANGUAGE plpgsql AS $$
DECLARE
    v_latest  integer;
    v_charts  bigint[];
    v_unknown text;
BEGIN
    IF p_dashboard_id IS NULL THEN
        IF p_based_on IS NOT NULL THEN
            RAISE EXCEPTION 'a new dashboard is not based on a revision'
                USING ERRCODE = 'invalid_parameter_value';
        END IF;
        INSERT INTO dashboard DEFAULT VALUES RETURNING dashboard.id INTO dashboard_id;
        revision := 1;
    ELSE
        PERFORM FROM dashboard d WHERE d.id = p_dashboard_id FOR UPDATE;
        IF NOT FOUND THEN
            RETURN;
        END IF;

        SELECT max(r.revision) INTO v_latest
        FROM dashboard_revision r WHERE r.dashboard_id = p_dashboard_id;
        IF p_based_on IS DISTINCT FROM v_latest THEN
            PERFORM raise_stale_save('dashboard', p_dashboard_id, p_based_on);
        END IF;
        dashboard_id := p_dashboard_id;
        revision := v_latest + 1;
    END IF;

    -- The insert checks the placements, so that reading them below is safe.
    BEGIN
        INSERT INTO dashboard_revision AS r (dashboard_id, revision, title, description, variables, placements)
        VALUES (save_dashboard.dashboard_id, save_dashboard.revision, p_title,
                coalesce(p_description, ''), coalesce(p_variables, '[]'), coalesce(p_placements, '[]'))
        RETURNING r.id INTO revision_id;
    EXCEPTION WHEN unique_violation THEN
        PERFORM raise_stale_save('dashboard', save_dashboard.dashboard_id, p_based_on);
    END;

    v_charts := ARRAY(
        SELECT DISTINCT (p -> 'chart' -> 'id')::numeric::bigint
        FROM jsonb_array_elements(coalesce(p_placements, '[]')) AS p
        WHERE p -> 'chart' ? 'id'
    );

    SELECT string_agg(u::text, ', ' ORDER BY u) INTO v_unknown
    FROM unnest(v_charts) AS u
    WHERE NOT EXISTS (SELECT 1 FROM chart c WHERE c.id = u);
    IF v_unknown IS NOT NULL THEN
        RAISE EXCEPTION 'dashboard places charts that do not exist: %', v_unknown
            USING ERRCODE = 'foreign_key_violation';
    END IF;

    DELETE FROM dashboard_chart u WHERE u.dashboard_id = save_dashboard.dashboard_id;
    INSERT INTO dashboard_chart (dashboard_id, chart_id)
    SELECT save_dashboard.dashboard_id, u FROM unnest(v_charts) AS u;
END
$$;

-- Deletes a dashboard with its revisions. Returns false if there is none.
-- The charts it places stay.
CREATE FUNCTION delete_dashboard(p_dashboard_id bigint) RETURNS boolean
    LANGUAGE plpgsql AS $$
BEGIN
    PERFORM FROM dashboard d WHERE d.id = p_dashboard_id FOR UPDATE;
    IF NOT FOUND THEN
        RETURN false;
    END IF;

    PERFORM set_config('track_record.deleting_dashboard', 'on', true);
    DELETE FROM dashboard d WHERE d.id = p_dashboard_id;
    PERFORM set_config('track_record.deleting_dashboard', 'off', true);
    RETURN true;
END
$$;
