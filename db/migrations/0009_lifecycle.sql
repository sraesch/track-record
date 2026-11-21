-- Run lifecycle functions that need every result table.

-- The last time anything was uploaded for a run, in server time: the run
-- itself, a case execution, a sample, a series or an attachment.
CREATE FUNCTION run_last_upload_at(p_run_id uuid) RETURNS timestamptz
    LANGUAGE sql STABLE AS $$
    SELECT greatest(
        (SELECT r.created_at FROM run r WHERE r.id = p_run_id),
        (SELECT max(ce.created_at) FROM case_execution ce WHERE ce.run_id = p_run_id),
        (SELECT max(s.created_at) FROM sample s
         JOIN case_execution ce ON ce.id = s.case_execution_id
         WHERE ce.run_id = p_run_id),
        (SELECT max(se.created_at) FROM series se
         JOIN sample s ON s.id = se.sample_id
         JOIN case_execution ce ON ce.id = s.case_execution_id
         WHERE ce.run_id = p_run_id),
        (SELECT max(a.created_at) FROM attachment a WHERE a.run_id = p_run_id)
    );
$$;

-- FR-2: marks running runs aborted that received no upload for longer than
-- p_timeout. Only server time counts. The measured start time (run.started_at)
-- is ignored, so a run that is uploaded hours after it was measured, or
-- imported much later, is not stale. Returns the ids of the aborted runs.
CREATE FUNCTION abort_stale_runs(p_timeout interval) RETURNS SETOF uuid
    LANGUAGE sql AS $$
    UPDATE run r
    SET status = 'aborted'
    WHERE r.status = 'running'
      AND r.created_at < now() - p_timeout
      AND run_last_upload_at(r.id) < now() - p_timeout
    RETURNING r.id;
$$;
