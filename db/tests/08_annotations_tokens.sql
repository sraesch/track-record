-- Tags, annotations (editable, no author) and API tokens (NFR-2).
BEGIN;
\ir lib/fixtures.sql
SELECT plan(12);

SELECT fx_new_run() AS run \gset

-- Tags ------------------------------------------------------------------------

SELECT lives_ok(
    format($$ INSERT INTO run_tag (run_id, tag) VALUES (%L, 'baseline') $$, :'run'),
    'a run can be tagged');
SELECT throws_ok(
    format($$ INSERT INTO run_tag (run_id, tag) VALUES (%L, 'baseline') $$, :'run'),
    '23505', NULL, 'a tag is set once per run');
SELECT throws_ok(
    format($$ INSERT INTO run_tag (run_id, tag) VALUES (%L, ' padded ') $$, :'run'),
    '23514', NULL, 'tags cannot have surrounding whitespace');
SELECT lives_ok(
    format($$ DELETE FROM run_tag WHERE run_id = %L $$, :'run'),
    'tags can be removed');

-- Annotations -----------------------------------------------------------------

SELECT throws_ok(
    $$ INSERT INTO annotation (body) VALUES ('orphan') $$,
    '23514', NULL, 'an annotation needs a target');
SELECT throws_ok(
    format($$ INSERT INTO annotation (run_id, occurred_at, body) VALUES (%L, now(), 'x') $$, :'run'),
    '23514', NULL, 'an annotation has exactly one target');
SELECT lives_ok(
    $$ INSERT INTO annotation (occurred_at, body, updated_at)
       VALUES (now(), 'switched to new compression', '2000-01-01') $$,
    'an annotation can mark a point in time');

UPDATE annotation SET body = 'switched to zstd compression';
SELECT is((SELECT updated_at FROM annotation), now(), 'editing an annotation updates updated_at');

-- API tokens ------------------------------------------------------------------

SELECT lives_ok(
    $$ INSERT INTO api_token (name, scope, token_hash) VALUES ('harness', 'write', sha256('secret')) $$,
    'a token is stored by its hash');
SELECT throws_ok(
    $$ INSERT INTO api_token (name, scope, token_hash) VALUES ('copy', 'read', sha256('secret')) $$,
    '23505', NULL, 'token hashes are unique');
SELECT throws_ok(
    $$ INSERT INTO api_token (name, scope, token_hash) VALUES ('short', 'read', '\x00'::bytea) $$,
    '23514', NULL, 'the stored hash must be a SHA-256');
SELECT throws_ok(
    $$ INSERT INTO api_token (name, scope, token_hash) VALUES ('admin', 'delete', sha256('a')) $$,
    '22P02', NULL, 'there is no scope beyond read, annotate and write');

SELECT * FROM finish();
ROLLBACK;
