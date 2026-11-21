-- Attachment metadata (ADR 0003). The files live on the shared volume under
-- runs/{run_id}/{attachment_id}; only the attachment store module touches them.
--
-- An attachment always belongs to a run (for deletion and the file path) and
-- optionally to one of its case executions or samples. The composite foreign
-- keys make sure those belong to the same run.

CREATE TABLE attachment (
    id                uuid PRIMARY KEY,
    run_id            uuid NOT NULL REFERENCES run ON DELETE CASCADE,
    case_execution_id uuid,
    sample_id         uuid,
    file_name         text NOT NULL CHECK (btrim(file_name) <> ''),
    content_type      text NOT NULL CHECK (btrim(content_type) <> ''),
    size_bytes        bigint NOT NULL CHECK (size_bytes >= 0),
    sha256            bytea NOT NULL CHECK (length(sha256) = 32),
    created_at        timestamptz NOT NULL DEFAULT now(),
    CHECK (sample_id IS NULL OR case_execution_id IS NOT NULL),
    FOREIGN KEY (run_id, case_execution_id)
        REFERENCES case_execution (run_id, id) ON DELETE CASCADE,
    FOREIGN KEY (case_execution_id, sample_id)
        REFERENCES sample (case_execution_id, id) ON DELETE CASCADE
);

CREATE INDEX attachment_run_idx ON attachment (run_id);
CREATE INDEX attachment_case_execution_idx ON attachment (case_execution_id);
CREATE INDEX attachment_sample_idx ON attachment (sample_id);

SELECT make_immutable('attachment', true);
