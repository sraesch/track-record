//! Ingestion (FR-1 to FR-4, FR-6): incremental upload, idempotent retries,
//! conflicts, run lifecycle, corrections and series validation.

mod common;

use reqwest::StatusCode;
use serde_json::{Value, json};
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn full_upload_then_read_back() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let (run_id, ce_id) = seed_run(&api, &app, "pixel-8-lab-1", "2026-11-20T22:00:00Z").await;

    // Incremental: one more sample, then one more series on it.
    let sample_id = Uuid::new_v4();
    let (status, body) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{ce_id}/samples/{sample_id}"
            )),
            &json!({"repetition": 2, "is_warmup": true, "metrics": metrics(),
                    "measurements": {"load_time": 2.5, "frames": 120}}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let series_id = Uuid::new_v4();
    let (status, body) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{ce_id}/samples/{sample_id}/series/{series_id}"
            )),
            &json!({"metrics": metrics(), "metric": "fps", "x_dimension": "time", "x_unit": "s",
                    "x": [0.0, 1.0], "y": [5.0, 6.5]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = api
        .post(
            &app.m(&format!("/runs/{run_id}/close")),
            &json!({"status": "completed"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, run) = api.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["status"], "completed");
    assert_eq!(run["suite"]["name"], "nightly");
    assert_eq!(run["sut_version"]["commit"], "abc123");
    assert_eq!(run["shared_environment"], json!({"server": "backend-1"}));
    assert_eq!(run["tags"], json!(["nightly"]));
    assert_eq!(run["case_execution_count"], 1);
    assert_eq!(run["started_at"], "2026-11-20T22:00:00Z");

    let (_, ces) = api
        .get(&app.m(&format!("/runs/{run_id}/case-executions")))
        .await;
    assert_eq!(ces.as_array().unwrap().len(), 1);
    assert_eq!(
        ces[0]["test_case"]["parameters"],
        json!({"size_mb": 10, "pipeline": "classic"})
    );
    assert_eq!(
        ces[0]["test_case"]["parameter_units"],
        json!({"size_mb": "MB"})
    );
    assert_eq!(ces[0]["test_case"]["assets"][0]["name"], "engine.glb");
    assert_eq!(ces[0]["client_device"]["name"], "pixel-8-lab-1");
    assert_eq!(ces[0]["sample_count"], 3);

    let (_, ce) = api.get(&app.m(&format!("/case-executions/{ce_id}"))).await;
    assert_eq!(ce["device_details"], json!({"os": "Android 15"}));
    let samples = ce["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0]["measurements"][0]["metric"], "load_time");
    assert_eq!(samples[0]["measurements"][0]["value"], 1.5);
    assert_eq!(samples[0]["measurements"][0]["unit"], "s");
    assert_eq!(samples[2]["is_warmup"], true);
    let frames = samples[2]["measurements"].as_array().unwrap();
    assert!(
        frames
            .iter()
            .any(|m| m["metric"] == "frames" && m["value"] == 120.0)
    );
    let series = &samples[0]["series"][0];
    assert_eq!(series["metric"], "fps");
    assert_eq!(series["point_count"], 5);

    let (_, points) = api
        .get(&app.m(&format!(
            "/series/{}/points",
            series["id"].as_str().unwrap()
        )))
        .await;
    assert_eq!(points["x"], json!([0.0, 0.5, 1.0, 1.0, 2.0]));
    assert_eq!(points["y"], json!([10.0, 30.0, 60.0, 55.0, 58.0]));
}

#[tokio::test]
async fn retries_are_idempotent_and_changes_conflict() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let run_id = Uuid::new_v4();
    let url = app.m(&format!("/runs/{run_id}"));
    let body = run_body("2026-11-20T22:00:00.1234567Z");
    assert_eq!(api.put(&url, &body).await.0, StatusCode::CREATED);
    // Same content (sub-microsecond digits are truncated in both): no-op.
    let (status, resp) = api.put(&url, &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["created"], false);
    // Different content under the same id.
    let mut changed = body.clone();
    changed["sut_version"]["commit"] = json!("def456");
    let (status, problem) = api.put(&url, &changed).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "conflict");
    // The rejected request left no definitions behind.
    let (_, versions) = api.get(&app.m("/sut-versions")).await;
    assert_eq!(versions.as_array().unwrap().len(), 1);

    let ce_id = Uuid::new_v4();
    let ce_url = app.m(&format!("/runs/{run_id}/case-executions/{ce_id}"));
    let ce = case_execution_body("dev", 10, json!([sample(0, 1.0)]));
    assert_eq!(api.put(&ce_url, &ce).await.0, StatusCode::CREATED);
    let (status, resp) = api.put(&ce_url, &ce).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["created"], false);

    // A changed measurement in the retried sample conflicts.
    let mut changed = ce.clone();
    changed["samples"][0]["measurements"]["load_time"] = json!(9.0);
    assert_eq!(api.put(&ce_url, &changed).await.0, StatusCode::CONFLICT);
    // So does a changed series point.
    let mut changed = ce.clone();
    changed["samples"][0]["series"][0]["y"][0] = json!(11.0);
    assert_eq!(api.put(&ce_url, &changed).await.0, StatusCode::CONFLICT);
    // A new sample under an existing case execution is added.
    let mut more = ce.clone();
    more["samples"].as_array_mut().unwrap().push(sample(1, 1.1));
    let (status, resp) = api.put(&ce_url, &more).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    // A different sample id with a taken repetition is a duplicate.
    let mut dup = ce.clone();
    dup["samples"] = json!([sample(0, 1.0)]);
    let (status, problem) = api.put(&ce_url, &dup).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "duplicate");
}

#[tokio::test]
async fn partial_results_survive_without_close() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let (run_id, _) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    // The harness "crashed": nothing closes the run. Everything sent so far
    // is stored and readable.
    let (_, run) = api.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["status"], "running");
    assert_eq!(run["case_execution_count"], 1);
}

#[tokio::test]
async fn run_lifecycle_rules() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let (run_id, ce_id) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    let close = app.m(&format!("/runs/{run_id}/close"));

    let (status, resp) = api.post(&close, &json!({"status": "failed"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["created"], true);
    // Same status again: no-op.
    let (status, resp) = api.post(&close, &json!({"status": "failed"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["created"], false);
    // Another terminal status: rejected.
    let (status, problem) = api.post(&close, &json!({"status": "completed"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "invalid_status_transition");
    // `aborted` is not something a client can send.
    assert_eq!(
        api.post(&close, &json!({"status": "aborted"})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Retrying a stored case execution still works after the close.
    let (_, ce) = api.get(&app.m(&format!("/case-executions/{ce_id}"))).await;
    assert_eq!(ce["samples"].as_array().unwrap().len(), 2);
    let (status, resp) = api
        .put(
            &app.m(&format!("/runs/{run_id}/case-executions/{ce_id}")),
            &case_execution_body("dev", 10, json!([])),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["created"], false);

    // New case executions need a running run ...
    let (status, problem) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{}",
                Uuid::new_v4()
            )),
            &case_execution_body("dev", 10, json!([])),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "run_not_running");

    // ... but a correction may be added to a finished run (FR-6).
    let correction_id = Uuid::new_v4();
    let mut correction = case_execution_body("dev", 10, json!([sample(0, 1.4)]));
    correction["supersedes_id"] = json!(ce_id);
    let (status, body) = api
        .put(
            &app.m(&format!("/runs/{run_id}/case-executions/{correction_id}")),
            &correction,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (_, ces) = api
        .get(&app.m(&format!("/runs/{run_id}/case-executions")))
        .await;
    let ces = ces.as_array().unwrap();
    assert_eq!(ces.len(), 1, "superseded case executions are hidden");
    assert_eq!(ces[0]["id"], json!(correction_id));
    assert_eq!(ces[0]["supersedes_id"], json!(ce_id));

    let (_, all) = api
        .get(&app.m(&format!(
            "/runs/{run_id}/case-executions?include_superseded=true"
        )))
        .await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    let old = all
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!(ce_id))
        .unwrap();
    assert_eq!(old["superseded_by"], json!(correction_id));

    // A correction must replace the same test case and device.
    let mut wrong = case_execution_body("other-device", 10, json!([]));
    wrong["supersedes_id"] = json!(correction_id);
    let (status, _) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{}",
                Uuid::new_v4()
            )),
            &wrong,
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn closing_or_adding_to_missing_resources_is_not_found() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let missing = Uuid::new_v4();
    let (status, _) = api
        .post(
            &app.m(&format!("/runs/{missing}/close")),
            &json!({"status": "completed"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api
        .put(
            &app.m(&format!(
                "/runs/{missing}/case-executions/{}",
                Uuid::new_v4()
            )),
            &case_execution_body("dev", 1, json!([])),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (run_id, ce_id) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    // A case execution of another run.
    let (other_run, _) = seed_run(&api, &app, "dev", "2026-11-20T23:00:00Z").await;
    let (status, _) = api
        .put(
            &app.m(&format!(
                "/runs/{other_run}/case-executions/{ce_id}/samples/{}",
                Uuid::new_v4()
            )),
            &json!({"repetition": 5}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A series on a sample that does not exist.
    let (status, _) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{ce_id}/samples/{}/series/{}",
                Uuid::new_v4(),
                Uuid::new_v4()
            )),
            &json!({"metrics": metrics(), "metric": "fps", "x_dimension": "time", "x_unit": "s",
                    "x": [], "y": []}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn put_series(app: &TestApp, api: &Api, body: Value) -> (StatusCode, Value) {
    let (run_id, ce_id) = seed_run(api, app, "dev", "2026-11-20T22:00:00Z").await;
    let sample_id = Uuid::new_v4();
    let (status, _) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{ce_id}/samples/{sample_id}"
            )),
            &json!({"repetition": 7}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    api.put(
        &app.m(&format!(
            "/runs/{run_id}/case-executions/{ce_id}/samples/{sample_id}/series/{}",
            Uuid::new_v4()
        )),
        &body,
    )
    .await
}

#[tokio::test]
async fn series_are_validated() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let series = |x: Value, y: Value| {
        json!({"metrics": metrics(), "metric": "fps", "x_dimension": "time", "x_unit": "s",
               "x": x, "y": y})
    };

    let (status, problem) =
        put_series(&app, &api, series(json!([1.0, 0.5]), json!([1.0, 2.0]))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["code"], "series_x_decreasing");

    let (status, _) = put_series(&app, &api, series(json!([1.0]), json!([1.0, 2.0]))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = put_series(&app, &api, series(json!([1.0]), json!([1e40]))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Undefined metric.
    let mut undefined = series(json!([1.0]), json!([1.0]));
    undefined["metric"] = json!("unknown");
    let (status, problem) = put_series(&app, &api, undefined).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(problem["detail"].as_str().unwrap().contains("unknown"));

    // A second series of the same metric on the same sample.
    let (run_id, ce_id) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    let (_, ce) = api.get(&app.m(&format!("/case-executions/{ce_id}"))).await;
    let sample_id = ce["samples"][0]["id"].as_str().unwrap();
    let (status, problem) = api
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{ce_id}/samples/{sample_id}/series/{}",
                Uuid::new_v4()
            )),
            &series(json!([0.0]), json!([1.0])),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "duplicate");

    // Large series in one request.
    let n = 10_000;
    let x: Vec<f64> = (0..n).map(|i| i as f64 * 0.01).collect();
    let y: Vec<f64> = (0..n).map(|i| (i % 60) as f64).collect();
    let (status, body) = put_series(&app, &api, series(json!(x), json!(y))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (_, points) = api
        .get(&app.m(&format!("/series/{}/points", body["id"].as_str().unwrap())))
        .await;
    assert_eq!(points["x"].as_array().unwrap().len(), n);
}

#[tokio::test]
async fn request_bodies_are_validated() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let url = app.m(&format!("/runs/{}", Uuid::new_v4()));

    let mut unknown_field = run_body("2026-11-20T22:00:00Z");
    unknown_field["colour"] = json!("blue");
    let (status, problem) = api.put(&url, &unknown_field).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["code"], "invalid_request_body");

    let mut bad_trigger = run_body("2026-11-20T22:00:00Z");
    bad_trigger["triggered_by"] = json!("cron");
    assert_eq!(
        api.put(&url, &bad_trigger).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let mut blank = run_body("2026-11-20T22:00:00Z");
    blank["suite"]["name"] = json!("  ");
    assert_eq!(
        api.put(&url, &blank).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (status, _) = send(
        api.client
            .put(&url)
            .bearer_auth(api.token.as_ref().unwrap())
            .header("content-type", "application/json")
            .body("{not json"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Parameters must be flat scalars; units only for numeric parameters.
    let (run_id, _) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    let ce_url = app.m(&format!(
        "/runs/{run_id}/case-executions/{}",
        Uuid::new_v4()
    ));
    let mut nested = case_execution_body("dev", 1, json!([]));
    nested["test_case"]["parameters"]["nested"] = json!({"a": 1});
    assert_eq!(
        api.put(&ce_url, &nested).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut unit_on_string = case_execution_body("dev", 1, json!([]));
    unit_on_string["test_case"]["parameter_units"]["pipeline"] = json!("MB");
    assert_eq!(
        api.put(&ce_url, &unit_on_string).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // A parameter name cannot contain a backtick: expressions quote names
    // with it.
    let mut backtick = case_execution_body("dev", 1, json!([]));
    backtick["test_case"]["parameters"]["tick`count"] = json!(3);
    let (status, resp) = api.put(&ce_url, &backtick).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert_eq!(resp["code"], "validation_failed");
    assert!(
        resp["detail"].as_str().unwrap().contains("backtick"),
        "{resp}"
    );
    // Integer metrics need integral values.
    let mut fractional = case_execution_body("dev", 1, json!([sample(0, 1.0)]));
    fractional["samples"][0]["measurements"]["frames"] = json!(1.5);
    assert_eq!(
        api.put(&ce_url, &fractional).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn parameters_keep_their_json_type() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let (run_id, _) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    for size in [json!(10), json!("10")] {
        let mut body = case_execution_body("dev", 0, json!([]));
        body["test_case"]["parameters"]["size_mb"] = size.clone();
        if size.is_string() {
            body["test_case"]["parameter_units"] = json!({});
        }
        let (status, resp) = api
            .put(
                &app.m(&format!(
                    "/runs/{run_id}/case-executions/{}",
                    Uuid::new_v4()
                )),
                &body,
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{resp}");
    }
    let (_, cases) = api.get(&app.m("/test-cases?scenario=load-model")).await;
    let sizes: Vec<&Value> = cases
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["parameters"]["size_mb"])
        .collect();
    assert!(sizes.contains(&&json!(10)));
    assert!(sizes.contains(&&json!("10")));
    // Same parameters, same test case: 10 (seeded) and 10 again.
    assert_eq!(sizes.len(), 2);
}

#[tokio::test]
async fn a_stored_parameter_name_with_a_backtick_can_be_retried() {
    let app = app!();
    let api = app.as_scope(TokenScope::Write).await;
    let (run_id, seeded) = seed_run(&api, &app, "dev", "2026-11-20T22:00:00Z").await;
    let mut body = case_execution_body("dev", 0, json!([]));
    body["test_case"]["parameters"]["tick`count"] = json!(3);

    // Stored by a release before the rule: the test case and case
    // execution as that release wrote them.
    let legacy = Uuid::new_v4();
    let test_case: i64 = sqlx::query_scalar(
        "SELECT ensure_test_case(ensure_scenario_version($1, $2), \
                ARRAY[ensure_asset('engine.glb', 'v1', NULL)], $3, $4)",
    )
    .bind(body["test_case"]["scenario"]["name"].as_str())
    .bind(&body["test_case"]["scenario"]["definition"])
    .bind(&body["test_case"]["parameters"])
    .bind(&body["test_case"]["parameter_units"])
    .fetch_one(&app.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO case_execution (id, run_id, test_case_id, client_device_id, \
                                     device_environment_id) \
         SELECT $1, run_id, $2, client_device_id, device_environment_id \
         FROM case_execution WHERE id = $3",
    )
    .bind(legacy)
    .bind(test_case)
    .bind(seeded)
    .execute(&app.pool)
    .await
    .unwrap();

    // An identical retry is still a 200; a new case execution is a 422.
    let url = |id: Uuid| app.m(&format!("/runs/{run_id}/case-executions/{id}"));
    let (status, resp) = api.put(&url(legacy), &body).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    let (status, resp) = api.put(&url(Uuid::new_v4()), &body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert!(
        resp["detail"].as_str().unwrap().contains("backtick"),
        "{resp}"
    );
}
