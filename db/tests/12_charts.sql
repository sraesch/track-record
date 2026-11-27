-- Charts and their history (FR-26, ADR 0022): every save adds a revision,
-- revisions never change, and a save on an old revision is rejected.
BEGIN;
SELECT plan(30);

\set spec1 '{"format": 1, "nodes": [{"id": "runs", "type": "source", "metric": "load_time"}]}'
\set spec2 '{"format": 1, "nodes": [{"id": "runs", "type": "source", "metric": "fps"}]}'

-- Creating ----------------------------------------------------------------------

SELECT chart_id AS c, revision, revision_id AS r1
FROM save_chart(NULL, NULL, 'Load time', 'Per release', :'spec1') \gset
SELECT is(:revision, 1, 'a new chart starts with revision 1');
SELECT results_eq(
    format($$ SELECT title, description, spec FROM current_chart_revision WHERE chart_id = %s $$, :c),
    format($$ VALUES ('Load time', 'Per release', %L::jsonb) $$, :'spec1'),
    'the first revision is the current definition');

SELECT throws_ok(
    format($$ SELECT save_chart(NULL, NULL, ' ', '', %L) $$, :'spec1'),
    '23514', NULL, 'a chart needs a title');
SELECT throws_ok(
    $$ SELECT save_chart(NULL, NULL, 'Bad', '', '[1, 2]') $$,
    '23514', NULL, 'a spec is a JSON object');
SELECT throws_ok(
    $$ SELECT save_chart(NULL, NULL, 'Bad', '', '{"nodes": []}') $$,
    '23514', NULL, 'a spec has a format');
SELECT throws_ok(
    format($$ SELECT save_chart(NULL, 1, 'New', '', %L) $$, :'spec1'),
    '22023', NULL, 'a new chart is not based on a revision');
SELECT is((SELECT count(*) FROM chart), 1::bigint, 'rejected creates leave no chart behind');

SELECT revision_id AS no_text FROM save_chart(NULL, NULL, 'No text', NULL, :'spec1') \gset
SELECT is((SELECT description FROM chart_revision WHERE id = :no_text), '',
    'a missing description is empty');

-- Saving ------------------------------------------------------------------------

SELECT revision, revision_id AS r2
FROM save_chart(:c, 1, 'Frame rate', '', :'spec2') \gset
SELECT is(:revision, 2, 'a save based on the latest revision adds the next one');
SELECT results_eq(
    format($$ SELECT revision, title, spec FROM current_chart_revision WHERE chart_id = %s $$, :c),
    format($$ VALUES (2, 'Frame rate', %L::jsonb) $$, :'spec2'),
    'the latest revision is the current definition');
SELECT results_eq(
    format($$ SELECT title, spec FROM chart_revision WHERE id = %s $$, :r1),
    format($$ VALUES ('Load time', %L::jsonb) $$, :'spec1'),
    'the earlier revision is kept as it was');

SELECT throws_ok(
    format($$ SELECT save_chart(%s, 1, 'Stale', '', %L) $$, :c, :'spec1'),
    'TR011', NULL, 'a save based on an old revision is rejected');
SELECT throws_ok(
    format($$ SELECT save_chart(%s, NULL, 'Blind', '', %L) $$, :c, :'spec1'),
    'TR011', NULL, 'a save on an existing chart must name its revision');
SELECT throws_ok(
    format($$ SELECT save_chart(%s, 3, 'Ahead', '', %L) $$, :c, :'spec1'),
    'TR011', NULL, 'a save based on a revision that does not exist yet is rejected');
SELECT is((SELECT count(*) FROM chart_revision WHERE chart_id = :c), 2::bigint,
    'rejected saves add no revision');

SELECT ok((SELECT chart_id IS NULL AND revision IS NULL AND revision_id IS NULL
           FROM save_chart(-1, 1, 'Nothing', '', :'spec1')),
    'saving a chart that does not exist returns nothing');

SELECT revision AS same FROM save_chart(:c, 2, 'Frame rate', '', :'spec2') \gset
SELECT is(:same, 3, 'saving unchanged content still adds a revision');

-- Restoring: the content of an old revision is saved as a new one.
SELECT revision AS restored FROM save_chart(:c, 3,
    (SELECT title FROM chart_revision WHERE id = :r1),
    (SELECT description FROM chart_revision WHERE id = :r1),
    (SELECT spec FROM chart_revision WHERE id = :r1)) \gset
SELECT is(:restored, 4, 'restoring adds a revision');
SELECT results_eq(
    format($$ SELECT title, description, spec FROM current_chart_revision WHERE chart_id = %s $$, :c),
    format($$ VALUES ('Load time', 'Per release', %L::jsonb) $$, :'spec1'),
    'after restoring, the old content is the current definition');
SELECT results_eq(
    format($$ SELECT revision FROM chart_revision WHERE chart_id = %s ORDER BY revision $$, :c),
    $$ VALUES (1), (2), (3), (4) $$,
    'the history only grows');

SELECT chart_id AS other, revision AS other_revision
FROM save_chart(NULL, NULL, 'Other', '', :'spec2') \gset
SELECT is(:other_revision, 1, 'each chart counts its own revisions');

-- Immutable revisions ---------------------------------------------------------------

SELECT throws_ok(
    format($$ UPDATE chart_revision SET title = 'Changed' WHERE id = %s $$, :r1),
    'TR001', NULL, 'a revision cannot be changed');
SELECT throws_ok(
    $$ TRUNCATE chart_revision CASCADE $$,
    'TR001', NULL, 'revisions cannot be truncated');
SELECT throws_ok(
    format($$ DELETE FROM chart_revision WHERE id = %s $$, :r1),
    'TR002', NULL, 'a revision cannot be deleted on its own');
SELECT throws_ok(
    format($$ DELETE FROM chart WHERE id = %s $$, :c),
    'TR002', NULL, 'a chart cannot be deleted outside delete_chart()');

-- Deleting ------------------------------------------------------------------------

SELECT ok(delete_chart(:c), 'delete_chart() deletes a chart');
SELECT is((SELECT count(*) FROM chart_revision WHERE chart_id = :c), 0::bigint,
    'deleting a chart removes its revisions');
SELECT is((SELECT count(*) FROM chart_revision WHERE chart_id = :other), 1::bigint,
    'other charts keep their revisions');
SELECT ok(NOT delete_chart(:c), 'deleting a chart that does not exist returns false');
SELECT throws_ok(
    format($$ DELETE FROM chart_revision WHERE chart_id = %s $$, :other),
    'TR002', NULL, 'after delete_chart(), revisions are protected again');

SELECT * FROM finish();
ROLLBACK;
