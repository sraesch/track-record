-- Runs and their results: case executions, samples and scalar measurements.
-- Results are immutable (FR-6) and use client-generated UUIDs as primary keys,
-- so retried uploads hit the existing row (FR-3).
--
-- Error codes (in addition to TR001/TR002 from 0001):
--   TR003  invalid run status transition
--   TR004  run is not running

-- Runs -----------------------------------------------------------------------

CREATE TABLE run (
    id                    uuid PRIMARY KEY,
    suite_version_id      bigint NOT NULL REFERENCES suite_version,
    sut_version_id        bigint NOT NULL REFERENCES sut_version,
    shared_environment_id bigint NOT NULL REFERENCES environment,
    triggered_by          run_trigger NOT NULL,
    status                run_status NOT NULL DEFAULT 'running',
    -- Measured times from the harness. They may lie far in the past: runs are
    -- also uploaded after the fact or imported. Trends and time filters use them.
    started_at            timestamptz NOT NULL,
    ended_at              timestamptz,
    is_valid              boolean NOT NULL DEFAULT true,
    invalid_reason        text,
    -- Upload time on the server. Staleness (FR-2) counts from here.
    created_at            timestamptz NOT NULL DEFAULT now(),
    CHECK ((status = 'running') = (ended_at IS NULL)),
    CHECK (ended_at >= started_at),
    -- FR-9: invalid runs need a reason; valid runs have none.
    CHECK (CASE WHEN is_valid THEN invalid_reason IS NULL
                ELSE btrim(coalesce(invalid_reason, '')) <> '' END)
);

CREATE INDEX run_started_at_idx ON run (started_at);
CREATE INDEX run_sut_version_idx ON run (sut_version_id);
CREATE INDEX run_running_idx ON run (created_at) WHERE status = 'running';

-- FR-2: runs start as 'running' and move once to a terminal status. Only the
-- status, end time and validity may change.
CREATE FUNCTION run_guard() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status <> 'running' THEN
            RAISE EXCEPTION 'a run must be created with status running, not %', NEW.status
                USING ERRCODE = 'TR003';
        END IF;
        RETURN NEW;
    END IF;

    IF (NEW.id, NEW.suite_version_id, NEW.sut_version_id, NEW.shared_environment_id,
        NEW.triggered_by, NEW.started_at, NEW.created_at)
       IS DISTINCT FROM
       (OLD.id, OLD.suite_version_id, OLD.sut_version_id, OLD.shared_environment_id,
        OLD.triggered_by, OLD.started_at, OLD.created_at) THEN
        RAISE EXCEPTION 'only status, ended_at and validity of a run can change'
            USING ERRCODE = 'TR001';
    END IF;

    IF NEW.status IS DISTINCT FROM OLD.status THEN
        IF OLD.status <> 'running' THEN
            RAISE EXCEPTION 'invalid run status transition % -> %', OLD.status, NEW.status
                USING ERRCODE = 'TR003';
        END IF;
        NEW.ended_at := coalesce(NEW.ended_at, greatest(now(), NEW.started_at));
    ELSIF NEW.ended_at IS DISTINCT FROM OLD.ended_at THEN
        RAISE EXCEPTION 'ended_at can only be set together with a terminal status'
            USING ERRCODE = 'TR001';
    END IF;

    RETURN NEW;
END
$$;

CREATE TRIGGER run_guard BEFORE INSERT OR UPDATE ON run
    FOR EACH ROW EXECUTE FUNCTION run_guard();

CREATE TRIGGER run_no_direct_delete BEFORE DELETE OR TRUNCATE ON run
    FOR EACH STATEMENT EXECUTE FUNCTION enforce_immutable('deletable_with_run');

-- Case executions ------------------------------------------------------------
-- One test case on one client device within a run. A correction (FR-6) is a
-- new case execution that supersedes an earlier one of the same run, test case
-- and device; current_case_execution hides superseded ones.

CREATE TABLE case_execution (
    id                    uuid PRIMARY KEY,
    run_id                uuid NOT NULL REFERENCES run ON DELETE CASCADE,
    test_case_id          bigint NOT NULL REFERENCES test_case,
    client_device_id      bigint NOT NULL REFERENCES client_device,
    device_environment_id bigint NOT NULL REFERENCES environment,
    supersedes_id         uuid UNIQUE,
    started_at            timestamptz,
    ended_at              timestamptz,
    created_at            timestamptz NOT NULL DEFAULT now(),
    CHECK (ended_at >= started_at),
    CHECK (supersedes_id <> id),
    UNIQUE (run_id, id),
    UNIQUE (run_id, test_case_id, client_device_id, id),
    FOREIGN KEY (run_id, test_case_id, client_device_id, supersedes_id)
        REFERENCES case_execution (run_id, test_case_id, client_device_id, id) ON DELETE CASCADE
);

CREATE INDEX case_execution_trend_idx ON case_execution (test_case_id, client_device_id);

-- New case executions need a running run (FR-1). Corrections may be added to
-- finished runs.
CREATE FUNCTION case_execution_requires_running_run() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    v_status run_status;
BEGIN
    IF NEW.supersedes_id IS NOT NULL THEN
        RETURN NEW;
    END IF;

    -- FOR SHARE blocks a concurrent status change until this insert commits.
    SELECT status INTO v_status FROM run WHERE id = NEW.run_id FOR SHARE;
    IF v_status IS DISTINCT FROM 'running' THEN
        RAISE EXCEPTION 'run % is %, not running', NEW.run_id, coalesce(v_status::text, 'missing')
            USING ERRCODE = 'TR004';
    END IF;
    RETURN NEW;
END
$$;

CREATE TRIGGER case_execution_requires_running_run BEFORE INSERT ON case_execution
    FOR EACH ROW EXECUTE FUNCTION case_execution_requires_running_run();

CREATE VIEW current_case_execution AS
SELECT ce.*
FROM case_execution ce
WHERE NOT EXISTS (SELECT 1 FROM case_execution c2 WHERE c2.supersedes_id = ce.id);

-- Samples and measurements ---------------------------------------------------

CREATE TABLE sample (
    id                uuid PRIMARY KEY,
    case_execution_id uuid NOT NULL REFERENCES case_execution ON DELETE CASCADE,
    repetition        integer NOT NULL CHECK (repetition >= 0),
    is_warmup         boolean NOT NULL DEFAULT false,
    created_at        timestamptz NOT NULL DEFAULT now(),
    UNIQUE (case_execution_id, repetition),
    UNIQUE (case_execution_id, id)
);

CREATE TABLE measurement (
    sample_id         uuid NOT NULL REFERENCES sample ON DELETE CASCADE,
    metric_version_id bigint NOT NULL REFERENCES metric_version,
    -- Finite values only: NaN sorts above Infinity in PostgreSQL.
    value             double precision NOT NULL CHECK (value > '-Infinity' AND value < 'Infinity'),
    PRIMARY KEY (sample_id, metric_version_id)
);

CREATE INDEX measurement_metric_version_idx ON measurement (metric_version_id);

SELECT make_immutable(t, true)
FROM unnest(ARRAY['case_execution', 'sample', 'measurement']::regclass[]) AS t;

-- Lifecycle functions --------------------------------------------------------
-- abort_stale_runs() is in 0009_lifecycle.sql: it looks at every result table.

-- FR-10: hard-deletes a run with all its results and attachment rows. The
-- caller removes the attachment files afterwards. Returns false if the run
-- does not exist. This is the only way to delete results.
CREATE FUNCTION delete_run(p_run_id uuid) RETURNS boolean
    LANGUAGE plpgsql AS $$
DECLARE
    v_found boolean;
BEGIN
    PERFORM set_config('track_record.deleting_run', 'on', true);
    DELETE FROM run WHERE id = p_run_id;
    v_found := FOUND;
    PERFORM set_config('track_record.deleting_run', 'off', true);
    RETURN v_found;
END
$$;
