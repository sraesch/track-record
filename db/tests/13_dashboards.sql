-- Dashboards and their history (FR-25, FR-26, ADR 0022): revisions as for
-- charts, and a chart that the current revision of a dashboard places
-- cannot be deleted.
BEGIN;
SELECT plan(36);

\set spec '{"format": 1, "nodes": []}'
\set vars '[{"name": "branch", "type": "text", "default": "main"}]'

SELECT chart_id AS a FROM save_chart(NULL, NULL, 'Load time', '', :'spec') \gset
SELECT chart_id AS b FROM save_chart(NULL, NULL, 'Frame rate', '', :'spec') \gset

-- Creating ----------------------------------------------------------------------

SELECT dashboard_id AS d, revision, revision_id AS r1
FROM save_dashboard(NULL, NULL, 'Release', 'Before a release', :'vars',
    format('[{"chart": {"id": %s}, "layout": {"x": 0, "y": 0, "w": 6, "h": 4},
              "variables": {"branch": {"var": "branch"}}},
             {"chart": {"builtin": "trends"}},
             {"chart": {"id": %s}, "variables": {"branch": "develop"}}]', :a, :a)::jsonb) \gset
SELECT is(:revision, 1, 'a new dashboard starts with revision 1');
SELECT results_eq(
    format($$ SELECT title, description, variables FROM current_dashboard_revision WHERE dashboard_id = %s $$, :d),
    format($$ VALUES ('Release', 'Before a release', %L::jsonb) $$, :'vars'),
    'the first revision is the current definition');
SELECT is((SELECT jsonb_array_length(placements) FROM dashboard_revision WHERE id = :r1), 3,
    'the placements are kept as saved, a chart may be placed twice');
SELECT results_eq(
    format($$ SELECT chart_id FROM dashboard_chart WHERE dashboard_id = %s $$, :d),
    format($$ VALUES (%s::bigint) $$, :a),
    'the saved charts it places are recorded once, built-in ones are not');

SELECT revision_id AS empty FROM save_dashboard(NULL, NULL, 'Empty', NULL, NULL, NULL) \gset
SELECT ok((SELECT variables = '[]' AND placements = '[]' FROM dashboard_revision WHERE id = :empty),
    'a dashboard can start without variables and charts');

-- What a dashboard can hold ------------------------------------------------------

SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, '', '', '[]', '[]') $$,
    '23514', NULL, 'a dashboard needs a title');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '{}', '[]') $$,
    '23514', NULL, 'variables are a list');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '{}') $$,
    '23514', NULL, 'placements are a list');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"layout": {}}]') $$,
    '23514', NULL, 'a placement names a chart');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": 1, "builtin": "trends"}}]') $$,
    '23514', NULL, 'a placement names a saved or a built-in chart, not both');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": "1"}}]') $$,
    '23514', NULL, 'the id of a chart is a number');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": 1.5}}]') $$,
    '23514', NULL, 'the id of a chart is a whole number');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": 0}}]') $$,
    '23514', NULL, 'the id of a chart is positive');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": 9223372036854775808}}]') $$,
    '23514', NULL, 'the id of a chart is in the range of bigint');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": 9223372036854775807}}]') $$,
    '23503', NULL, 'the largest bigint is a valid id, here of a chart that does not exist');
SELECT throws_ok(
    $$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"builtin": " "}}]') $$,
    '23514', NULL, 'a built-in chart has a name');
SELECT throws_ok(
    format($$ SELECT save_dashboard(NULL, NULL, 'Bad', '', '[]', '[{"chart": {"id": %s}}]') $$, :b + 1000),
    '23503', NULL, 'a placed chart must exist');
SELECT is((SELECT count(*) FROM dashboard), 2::bigint, 'rejected creates leave no dashboard behind');

-- Charts in use ---------------------------------------------------------------------

SELECT throws_ok(
    format($$ SELECT delete_chart(%s) $$, :a),
    'TR012', NULL, 'a chart that a dashboard places cannot be deleted');
SELECT is((SELECT count(*) FROM chart_revision WHERE chart_id = :a), 1::bigint,
    'the rejected delete leaves the chart as it was');
SELECT throws_ok(
    format($$ SELECT set_config('track_record.deleting_chart', 'on', true);
              DELETE FROM chart WHERE id = %s $$, :a),
    '23503', NULL, 'outside delete_chart(), the foreign key protects a chart in use');
SELECT ok(delete_chart(:b), 'a chart that no dashboard places can be deleted');

-- Saving ------------------------------------------------------------------------

SELECT throws_ok(
    format($$ SELECT save_dashboard(%s, 0, 'Stale', '', '[]', '[]') $$, :d),
    'TR011', NULL, 'a save based on an old revision is rejected');

SELECT revision FROM save_dashboard(:d, 1, 'Release', 'Before a release', :'vars',
    '[{"chart": {"builtin": "trends"}}]') \gset
SELECT is(:revision, 2, 'a save adds the next revision');
SELECT is((SELECT count(*) FROM dashboard_chart WHERE dashboard_id = :d), 0::bigint,
    'the recorded charts follow the current revision');
SELECT ok(delete_chart(:a), 'a chart that only an old revision placed can be deleted');

SELECT throws_ok(
    format($$ SELECT save_dashboard(%s, 2, title, description, variables, placements)
              FROM dashboard_revision WHERE id = %s $$, :d, :r1),
    '23503', NULL, 'restoring a revision that places a deleted chart is rejected');
SELECT is((SELECT max(revision) FROM dashboard_revision WHERE dashboard_id = :d), 2,
    'the rejected restore adds no revision');

-- Restoring a revision whose charts still exist places them again.
SELECT chart_id AS e FROM save_chart(NULL, NULL, 'Error rate', '', :'spec') \gset
SELECT dashboard_id AS d2, revision_id AS d2r1
FROM save_dashboard(NULL, NULL, 'Errors', '', '[]', format('[{"chart": {"id": %s}}]', :e)::jsonb) \gset
SELECT revision FROM save_dashboard(:d2, 1, 'Errors', '', '[]', '[]') \gset
SELECT s.revision
FROM dashboard_revision r,
     LATERAL save_dashboard(:d2, 2, r.title, r.description, r.variables, r.placements) s
WHERE r.id = :d2r1 \gset
SELECT results_eq(
    format($$ SELECT revision, placements FROM current_dashboard_revision WHERE dashboard_id = %s $$, :d2),
    format($$ VALUES (3, '[{"chart": {"id": %s}}]'::jsonb) $$, :e),
    'restoring adds a revision with the old placements');
SELECT throws_ok(
    format($$ SELECT delete_chart(%s) $$, :e),
    'TR012', NULL, 'after restoring, the placed chart is protected again');

SELECT ok((SELECT dashboard_id IS NULL FROM save_dashboard(-1, 1, 'Nothing', '', '[]', '[]')),
    'saving a dashboard that does not exist returns nothing');

-- Immutable revisions ---------------------------------------------------------------

SELECT throws_ok(
    format($$ UPDATE dashboard_revision SET title = 'Changed' WHERE id = %s $$, :r1),
    'TR001', NULL, 'a revision cannot be changed');
SELECT throws_ok(
    format($$ DELETE FROM dashboard WHERE id = %s $$, :d),
    'TR002', NULL, 'a dashboard cannot be deleted outside delete_dashboard()');

-- Deleting ------------------------------------------------------------------------

SELECT chart_id AS c FROM save_chart(NULL, NULL, 'Placed', '', :'spec') \gset
SELECT revision AS r3 FROM save_dashboard(:d, 2, 'Release', '', '[]',
    format('[{"chart": {"id": %s}}]', :c)::jsonb) \gset
SELECT ok(delete_dashboard(:d), 'delete_dashboard() deletes a dashboard');
SELECT is((SELECT count(*) FROM dashboard_revision WHERE dashboard_id = :d)
        + (SELECT count(*) FROM dashboard_chart WHERE dashboard_id = :d), 0::bigint,
    'deleting a dashboard removes its revisions and the record of its charts');
SELECT ok(delete_chart(:c), 'the charts of a deleted dashboard stay and can then be deleted');

SELECT * FROM finish();
ROLLBACK;
