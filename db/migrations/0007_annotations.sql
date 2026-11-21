-- Tags and annotations. Unlike results these are editable, through the web UI
-- and through MCP with an 'annotate' token. They have no author (NFR-2).

CREATE TABLE run_tag (
    run_id     uuid NOT NULL REFERENCES run ON DELETE CASCADE,
    tag        text NOT NULL CHECK (tag <> '' AND tag = btrim(tag)),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, tag)
);

CREATE INDEX run_tag_tag_idx ON run_tag (tag);

CREATE TABLE test_case_tag (
    test_case_id bigint NOT NULL REFERENCES test_case,
    tag          text NOT NULL CHECK (tag <> '' AND tag = btrim(tag)),
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (test_case_id, tag)
);

CREATE INDEX test_case_tag_tag_idx ON test_case_tag (tag);

-- An annotation is attached to exactly one of: a run, a test case, or a point
-- in time (e.g. "switched to new compression").
CREATE TABLE annotation (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id       uuid REFERENCES run ON DELETE CASCADE,
    test_case_id bigint REFERENCES test_case,
    occurred_at  timestamptz,
    body         text NOT NULL CHECK (btrim(body) <> ''),
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    CHECK (num_nonnulls(run_id, test_case_id, occurred_at) = 1)
);

CREATE INDEX annotation_run_idx ON annotation (run_id);
CREATE INDEX annotation_test_case_idx ON annotation (test_case_id);
CREATE INDEX annotation_occurred_at_idx ON annotation (occurred_at);

CREATE TRIGGER annotation_touch BEFORE UPDATE ON annotation
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
