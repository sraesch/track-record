-- Two saves of the same chart or dashboard at the same time (FR-26, ADR 0022):
-- the later one is rejected as stale (TR011), also under REPEATABLE READ,
-- where its snapshot still shows the revision it is based on as the latest.
--
-- Two other sessions (dblink) do the saves, so their rows are committed.
-- This file runs last and removes them again.
BEGIN;
SELECT plan(5);

CREATE EXTENSION dblink;

DO $$
BEGIN
    PERFORM dblink_connect(s, 'dbname=' || current_database())
    FROM unnest(ARRAY['setup', 'first', 'second']) AS s;
END
$$;

SELECT c FROM dblink('setup',
    $$ SELECT chart_id FROM save_chart(NULL, NULL, 'Race', '', '{"format": 1, "nodes": []}') $$)
    AS t(c bigint) \gset
SELECT d FROM dblink('setup',
    $$ SELECT dashboard_id FROM save_dashboard(NULL, NULL, 'Race', '', '[]', '[]') $$)
    AS t(d bigint) \gset

-- Runs a statement in the session and returns its SQLSTATE, or 'ok'.
DO $$
BEGIN
    PERFORM dblink_exec(s, $f$
        CREATE FUNCTION pg_temp.sqlstate_of(p_sql text) RETURNS text
            LANGUAGE plpgsql AS $b$
        BEGIN
            EXECUTE p_sql;
            RETURN 'ok';
        EXCEPTION WHEN OTHERS THEN
            RETURN SQLSTATE;
        END
        $b$ $f$)
    FROM unnest(ARRAY['first', 'second']) AS s;
END
$$;

-- Both sessions start from revision 1. The second saves first and commits;
-- the first then saves on revision 1 as well, which is no longer the latest.
CREATE FUNCTION pg_temp.race(p_save text) RETURNS text
    LANGUAGE plpgsql AS $$
DECLARE
    v_state text;
BEGIN
    PERFORM dblink_exec(s, 'BEGIN ISOLATION LEVEL REPEATABLE READ')
    FROM unnest(ARRAY['first', 'second']) AS s;
    -- Takes the snapshot of each session.
    PERFORM * FROM dblink('first', 'SELECT count(*) FROM chart_revision') AS t(n bigint);
    PERFORM * FROM dblink('second', 'SELECT count(*) FROM chart_revision') AS t(n bigint);

    SELECT t.state INTO v_state
    FROM dblink('second', format('SELECT pg_temp.sqlstate_of(%L)', p_save)) AS t(state text);
    IF v_state <> 'ok' THEN
        RAISE EXCEPTION 'the first save failed with %', v_state;
    END IF;
    -- Waits for the lock that the second session holds.
    PERFORM dblink_send_query('first', format('SELECT pg_temp.sqlstate_of(%L)', p_save));
    PERFORM dblink_exec('second', 'COMMIT');

    SELECT t.state INTO v_state FROM dblink_get_result('first') AS t(state text);
    PERFORM * FROM dblink_get_result('first') AS t(state text);
    PERFORM dblink_exec('first', 'COMMIT');
    RETURN v_state;
END
$$;

SELECT is(pg_temp.race(format(
        $$ SELECT save_chart(%s, 1, 'Mine', '', '{"format": 1, "nodes": []}') $$, :c)),
    'TR011', 'a concurrent save of a chart on the same revision is rejected as stale');
SELECT is(
    (SELECT n FROM dblink('setup', format('SELECT count(*) FROM chart_revision WHERE chart_id = %s', :c))
        AS t(n bigint)),
    2::bigint, 'only the first of the two saves added a revision');

SELECT is(pg_temp.race(format(
        $$ SELECT save_dashboard(%s, 1, 'Mine', '', '[]', '[]') $$, :d)),
    'TR011', 'a concurrent save of a dashboard on the same revision is rejected as stale');
SELECT is(
    (SELECT n FROM dblink('setup', format('SELECT count(*) FROM dashboard_revision WHERE dashboard_id = %s', :d))
        AS t(n bigint)),
    2::bigint, 'only the first of the two dashboard saves added a revision');

-- Clean up what the other sessions committed.
SELECT ok(dashboard_deleted AND chart_deleted, 'the committed rows are removed again')
FROM dblink('setup', format('SELECT delete_dashboard(%s), delete_chart(%s)', :d, :c))
    AS t(dashboard_deleted boolean, chart_deleted boolean);
DO $$
BEGIN
    PERFORM dblink_disconnect(s) FROM unnest(ARRAY['setup', 'first', 'second']) AS s;
END
$$;

SELECT * FROM finish();
ROLLBACK;
