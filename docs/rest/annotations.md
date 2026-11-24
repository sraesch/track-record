# Tags and annotations

Tags and annotations are the only data a client can edit after upload.
Unlike results they can be changed and removed. They have no author: there
are no users in track-record (NFR-2).

Writing needs a token with the **`annotate`** scope; reading works with any
token. A `write` token cannot change tags or annotations (it can only set a
run's initial tags when [creating the run](ingestion.md#create-a-run)).

## Tags

Tags are short labels on runs and on test cases, e.g. `nightly`,
`baseline`, `smoke`. A tag is a non-empty string; surrounding whitespace is
trimmed. Runs can be [filtered](analysis.md#run-filter) by tag; test case
tags are shown with the test case.

The tag is a path segment: percent-encode characters such as `/`, `?`,
`#`, `%` and spaces (`release%2F2.4`).

### Tag a run

```http
PUT /api/v1/runs/{run_id}/tags/{tag}
```

Adds the tag. Adding a tag the run already has is a no-op. Answers
`204 No Content`.

```bash
curl -s -X PUT "$TR_URL/api/v1/runs/0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01/tags/baseline" \
  -H "Authorization: Bearer $TR_TOKEN"
```

| Status | Code | When |
|---|---|---|
| `204` | | Added, or already there |
| `404` | `not_found` | No such run |
| `422` | `validation_failed` | Empty tag |

### Remove a tag from a run

```http
DELETE /api/v1/runs/{run_id}/tags/{tag}
```

Removes the tag. Answers `204`, also if the run did not have it;
`404 not_found` if the run does not exist.

### Tag a test case

```http
PUT /api/v1/test-cases/{test_case_id}/tags/{tag}
```

Adds a tag to a test case (ids from [test cases](definitions.md#list-test-cases)).
Same behaviour as for runs.

### Remove a tag from a test case

```http
DELETE /api/v1/test-cases/{test_case_id}/tags/{tag}
```

Same behaviour as for runs.

## Annotations

An annotation is a free-text note on **exactly one** of:

- a **run** (`run_id`), e.g. "render server was restarted before this run";
- a **test case** (`test_case_id`), e.g. "asset replaced on 2026-11-02";
- a **point in time** (`occurred_at`), e.g. "switched CDN to the new
  region". Trends show point-in-time annotations that fall within them.

`Annotation`:

| Field | Type | Meaning |
|---|---|---|
| `id` | UUID | Assigned by the server |
| `run_id` | UUID \| null | Annotated run |
| `test_case_id` | integer \| null | Annotated test case |
| `occurred_at` | timestamp \| null | Annotated point in time |
| `body` | string | The text (trimmed, non-empty). Plain text; the web UI shows it as is |
| `created_at` | timestamp | Server time |
| `updated_at` | timestamp | Server time of the last edit |

### List annotations

```http
GET /api/v1/annotations
```

| Parameter | Type | Meaning |
|---|---|---|
| `run_id` | UUID | Only annotations on this run |
| `test_case_id` | integer | Only annotations on this test case |
| `from` | timestamp | Only point-in-time annotations at or after this time |
| `to` | timestamp | Only point-in-time annotations before this time |

All given conditions must hold; without parameters, all annotations are
returned. `from`/`to` select point-in-time annotations only. Sorted by
`occurred_at` (or `created_at` for annotations without it), then id.

```bash
curl -s "$TR_URL/api/v1/annotations?from=2026-11-20T00:00:00Z&to=2026-11-21T00:00:00Z" \
  -H "Authorization: Bearer $TR_TOKEN"
```

```json
[
  {
    "id": "69bd03f0-ee0b-448f-94f0-8e46d39123fb",
    "run_id": null,
    "test_case_id": null,
    "occurred_at": "2026-11-20T12:00:00Z",
    "body": "Switched CDN to the new region.",
    "created_at": "2026-11-23T19:57:13.708948Z",
    "updated_at": "2026-11-23T19:57:13.708948Z"
  }
]
```

### Create an annotation

```http
POST /api/v1/annotations
```

Request body `CreateAnnotation`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `body` | string | yes | The text |
| `run_id` | UUID \| null | exactly one of the three | Annotate a run |
| `test_case_id` | integer \| null | | Annotate a test case |
| `occurred_at` | timestamp \| null | | Annotate a point in time |

Creating an annotation is not idempotent: every request creates a new one.

```bash
curl -s -X POST "$TR_URL/api/v1/annotations" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01", "body": "Render server was restarted before this run."}'
```

`201 Created`:

```json
{
  "id": "6f2a9763-671e-493d-8407-e3af3a52755d",
  "run_id": "0f8b6f2e-4d1a-4c3e-9a57-2b8f3c1d9e01",
  "test_case_id": null,
  "occurred_at": null,
  "body": "Render server was restarted before this run.",
  "created_at": "2026-11-23T19:57:13.704329Z",
  "updated_at": "2026-11-23T19:57:13.704329Z"
}
```

| Status | Code | When |
|---|---|---|
| `201` | | Created |
| `404` | `not_found` | The run or test case does not exist |
| `422` | `validation_failed` | Empty body, or not exactly one target (`an annotation needs exactly one of run_id, test_case_id and occurred_at`) |

### Get an annotation

```http
GET /api/v1/annotations/{annotation_id}
```

Response: one `Annotation`. `404 not_found` if it does not exist.

### Edit an annotation

```http
PATCH /api/v1/annotations/{annotation_id}
```

Replaces the text. The target cannot be changed; delete the annotation and
create a new one instead.

Request body `UpdateAnnotation`: `{"body": "..."}` (non-empty).

```bash
curl -s -X PATCH "$TR_URL/api/v1/annotations/6f2a9763-671e-493d-8407-e3af3a52755d" \
  -H "Authorization: Bearer $TR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"body": "Render server was restarted before this run; first samples are slow."}'
```

Response: the updated `Annotation` with a new `updated_at`.
`404 not_found` if it does not exist, `422 validation_failed` for an empty
body.

### Delete an annotation

```http
DELETE /api/v1/annotations/{annotation_id}
```

Deletes it for good. Answers `204`; `404 not_found` if it does not exist.
