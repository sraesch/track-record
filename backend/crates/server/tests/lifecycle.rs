//! Run lifecycle: invalidation (FR-9), hard delete (FR-10) and stale-run
//! abort (FR-2).

mod common;

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use track_record_server::db::TokenScope;
use track_record_server::service::lifecycle;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn invalid_runs_are_hidden_by_default_and_can_be_restored() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let (run_id, _) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;

    let url = app.u(&format!("/runs/{run_id}/invalidate"));
    assert_eq!(
        ui.post(&url, &json!({"reason": " "})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        ui.post(&url, &json!({"reason": "wrong build"})).await.0,
        StatusCode::NO_CONTENT
    );

    let (_, run) = ui.get(&app.u(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["is_valid"], false);
    assert_eq!(run["invalid_reason"], "wrong build");
    let (_, list) = ui.get(&app.u("/runs")).await;
    assert!(list["runs"].as_array().unwrap().is_empty());
    let (_, list) = ui.get(&app.u("/runs?validity=invalid")).await;
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    let (_, list) = ui.get(&app.u("/runs?validity=all")).await;
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);

    assert_eq!(
        ui.post(&app.u(&format!("/runs/{run_id}/validate")), &json!(null))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, run) = ui.get(&app.u(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["is_valid"], true);
    assert!(run["invalid_reason"].is_null());

    let missing = Uuid::new_v4();
    let (status, _) = ui
        .post(&app.u(&format!("/runs/{missing}/validate")), &json!(null))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn hard_delete_needs_confirmation_and_removes_everything() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let (run_id, _) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;
    let (keep_id, _) = seed_run(&write, &app, "dev", "2026-11-20T23:00:00Z").await;
    let response = write
        .raw(
            app.client
                .put(app.m(&format!(
                    "/runs/{run_id}/attachments/{}?file_name=log.txt",
                    Uuid::new_v4()
                )))
                .header("content-type", "text/plain")
                .body("log"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let run_dir = app
        .attachment_dir
        .path()
        .join("runs")
        .join(run_id.to_string());
    assert!(run_dir.exists());

    let url = app.u(&format!("/runs/{run_id}/delete"));
    let (status, problem) = ui.post(&url, &json!({"confirm_run_id": keep_id})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        problem["detail"]
            .as_str()
            .unwrap()
            .contains("confirm_run_id")
    );
    assert_eq!(
        ui.post(&url, &json!({})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (status, _) = ui.post(&url, &json!({"confirm_run_id": run_id})).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        ui.get(&app.u(&format!("/runs/{run_id}"))).await.0,
        StatusCode::NOT_FOUND
    );
    assert!(!run_dir.exists());
    let points: i64 = sqlx::query_scalar("SELECT count(*) FROM series_point")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(points, 10, "only the other run's points remain");
    assert_eq!(
        ui.get(&app.u(&format!("/runs/{keep_id}"))).await.0,
        StatusCode::OK
    );

    // Deleting again: not found.
    assert_eq!(
        ui.post(&url, &json!({"confirm_run_id": run_id})).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn stale_runs_are_aborted_by_server_time_only() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    // Measured two years ago, uploaded now (import): not stale.
    let (imported, _) = seed_run(&write, &app, "dev", "2024-11-20T22:00:00Z").await;
    let aborted = lifecycle::abort_stale_runs(&app.pool, Duration::from_secs(3600))
        .await
        .unwrap();
    assert!(aborted.is_empty());

    // With a short timeout and no further uploads, it is stale.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let aborted = lifecycle::abort_stale_runs(&app.pool, Duration::from_millis(100))
        .await
        .unwrap();
    assert_eq!(aborted, vec![imported]);
    let (_, run) = write.get(&app.m(&format!("/runs/{imported}"))).await;
    assert_eq!(run["status"], "aborted");

    // A run that keeps receiving uploads is not aborted.
    let (active, ce_id) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (status, _) = write
        .put(
            &app.m(&format!(
                "/runs/{active}/case-executions/{ce_id}/samples/{}",
                Uuid::new_v4()
            )),
            &json!({"repetition": 9}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let aborted = lifecycle::abort_stale_runs(&app.pool, Duration::from_millis(250))
        .await
        .unwrap();
    assert!(aborted.is_empty(), "the sample upload counts as activity");

    // An aborted run cannot be completed afterwards.
    let (status, _) = write
        .post(
            &app.m(&format!("/runs/{imported}/close")),
            &json!({"status": "completed"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
