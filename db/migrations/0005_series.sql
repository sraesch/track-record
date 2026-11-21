-- Series (ADR 0002): metadata in series, one row per point in series_point.
-- Only the backend's series store module reads or writes series_point.
--
-- Error codes:
--   TR005  series points don't match point_count
--   TR006  series x values decrease
--
-- Only raw, measured series are uploaded, one per metric and sample. A curve
-- over another x-dimension (e.g. quality over bytes) is a derived metric that
-- re-keys one series by another (FR-17), not a second stored series.
--
-- Completeness: a series and all its points are written in one transaction.
-- At commit, the series must have exactly point_count points at positions
-- 0..point_count-1, and x must never decrease from one position to the next
-- (equal x values are allowed). Points added later are rejected because their
-- position would be out of range or already taken.

CREATE TABLE series (
    id                uuid PRIMARY KEY,
    -- Compact internal key so point rows stay small.
    key               bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    sample_id         uuid NOT NULL REFERENCES sample ON DELETE CASCADE,
    metric_version_id bigint NOT NULL REFERENCES metric_version,
    x_dimension       text NOT NULL CHECK (btrim(x_dimension) <> ''),
    x_unit            text NOT NULL,  -- '' for dimensionless; see unit (FR-13a)
    point_count       integer NOT NULL CHECK (point_count >= 0),
    created_at        timestamptz NOT NULL DEFAULT now(),
    UNIQUE (sample_id, metric_version_id)
);

CREATE INDEX series_metric_version_idx ON series (metric_version_id);

CREATE TABLE series_point (
    series_key bigint NOT NULL REFERENCES series (key) ON DELETE CASCADE,
    position   integer NOT NULL CHECK (position >= 0),
    x          real NOT NULL CHECK (x > '-Infinity' AND x < 'Infinity'),
    y          real NOT NULL CHECK (y > '-Infinity' AND y < 'Infinity'),
    PRIMARY KEY (series_key, position)
);

SELECT make_immutable(t, true)
FROM unnest(ARRAY['series', 'series_point']::regclass[]) AS t;

-- Checked at commit for every new series.
CREATE FUNCTION check_series_complete() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    v_count bigint;
BEGIN
    -- The series may already be gone again (deleted with its run in the same
    -- transaction).
    IF NOT EXISTS (SELECT 1 FROM series WHERE key = NEW.key) THEN
        RETURN NULL;
    END IF;

    SELECT count(*) INTO v_count FROM series_point WHERE series_key = NEW.key;
    IF v_count <> NEW.point_count THEN
        RAISE EXCEPTION 'series % has % points, expected %', NEW.id, v_count, NEW.point_count
            USING ERRCODE = 'TR005';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM (SELECT x, lag(x) OVER (ORDER BY position) AS previous_x
              FROM series_point WHERE series_key = NEW.key) p
        WHERE p.x < p.previous_x
    ) THEN
        RAISE EXCEPTION 'x values of series % decrease', NEW.id
            USING ERRCODE = 'TR006';
    END IF;
    RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER series_complete AFTER INSERT ON series
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION check_series_complete();

-- Checked per INSERT/COPY statement on series_point.
CREATE FUNCTION check_point_positions() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    v_bad record;
BEGIN
    SELECT s.id, p.position, s.point_count INTO v_bad
    FROM new_points p
    JOIN series s ON s.key = p.series_key
    WHERE p.position >= s.point_count
    LIMIT 1;

    IF FOUND THEN
        RAISE EXCEPTION 'point position % is out of range for series % with % points',
            v_bad.position, v_bad.id, v_bad.point_count
            USING ERRCODE = 'TR005';
    END IF;
    RETURN NULL;
END
$$;

CREATE TRIGGER series_point_positions AFTER INSERT ON series_point
    REFERENCING NEW TABLE AS new_points
    FOR EACH STATEMENT EXECUTE FUNCTION check_point_positions();
