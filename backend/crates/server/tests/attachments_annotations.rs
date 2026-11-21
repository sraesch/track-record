//! Attachments (FR-5, ADR 0003), tags and annotations.

mod common;

use reqwest::StatusCode;
use serde_json::json;
use sha2::{Digest, Sha256};
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn attachments_upload_retry_conflict_and_download() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let (run_id, ce_id) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;
    let id = Uuid::new_v4();
    let content = b"<html><script>alert(1)</script></html>".to_vec();
    let url = app.m(&format!(
        "/runs/{run_id}/attachments/{id}?file_name=report.html&case_execution_id={ce_id}"
    ));
    let upload = |body: Vec<u8>, sha: Option<String>| {
        let mut rb = app
            .client
            .put(&url)
            .header("content-type", "text/html")
            .body(body);
        if let Some(sha) = sha {
            rb = rb.header("x-content-sha256", sha);
        }
        rb
    };

    // Checksum mismatch is rejected and leaves nothing.
    let response = write
        .raw(upload(content.clone(), Some(hex::encode([0u8; 32]))))
        .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let sha = hex::encode(Sha256::digest(&content));
    let response = write.raw(upload(content.clone(), Some(sha.clone()))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    // Retry: no-op. Other content: conflict.
    assert_eq!(
        write.raw(upload(content.clone(), None)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        write.raw(upload(b"other".to_vec(), None)).await.status(),
        StatusCode::CONFLICT
    );

    let (_, meta) = write.get(&app.m(&format!("/attachments/{id}"))).await;
    assert_eq!(meta["file_name"], "report.html");
    assert_eq!(meta["size_bytes"], content.len());
    assert_eq!(meta["sha256"], sha);
    assert_eq!(meta["case_execution_id"], json!(ce_id));
    let (_, run) = write.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["attachments"].as_array().unwrap().len(), 1);

    let response = write
        .raw(app.client.get(app.m(&format!("/attachments/{id}/content"))))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers().clone();
    assert_eq!(headers["content-type"], "text/html");
    assert!(
        headers["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(response.bytes().await.unwrap().to_vec(), content);

    // Stored under its id, never under the uploaded name; no temp files.
    let dir = app
        .attachment_dir
        .path()
        .join("runs")
        .join(run_id.to_string());
    let names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec![id.to_string()]);

    // Owners must belong together.
    let bad = app.m(&format!(
        "/runs/{run_id}/attachments/{}?file_name=x&sample_id={}",
        Uuid::new_v4(),
        Uuid::new_v4()
    ));
    assert_eq!(
        write.raw(app.client.put(bad).body("x")).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (other_run, _) = seed_run(&write, &app, "dev", "2026-11-21T22:00:00Z").await;
    let bad = app.m(&format!(
        "/runs/{other_run}/attachments/{}?file_name=x&case_execution_id={ce_id}",
        Uuid::new_v4()
    ));
    assert_eq!(
        write.raw(app.client.put(bad).body("x")).await.status(),
        StatusCode::NOT_FOUND
    );
    let no_name = app.m(&format!("/runs/{run_id}/attachments/{}", Uuid::new_v4()));
    assert_eq!(
        write.raw(app.client.put(no_name).body("x")).await.status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn tags_and_annotations() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let annotate = app.as_scope(TokenScope::Annotate).await;
    let (run_id, _) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;

    // Run tags: add twice (no-op), remove.
    let tag = app.m(&format!("/runs/{run_id}/tags/regression"));
    assert_eq!(annotate.put_empty(&tag).await.0, StatusCode::NO_CONTENT);
    assert_eq!(annotate.put_empty(&tag).await.0, StatusCode::NO_CONTENT);
    let (_, run) = annotate.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["tags"], json!(["nightly", "regression"]));
    assert_eq!(annotate.delete(&tag).await.0, StatusCode::NO_CONTENT);
    let (_, run) = annotate.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["tags"], json!(["nightly"]));
    let missing = app.m(&format!("/runs/{}/tags/x", Uuid::new_v4()));
    assert_eq!(annotate.put_empty(&missing).await.0, StatusCode::NOT_FOUND);

    // Test case tags.
    let (_, cases) = annotate.get(&app.m("/test-cases")).await;
    let tc = cases[0]["id"].as_i64().unwrap();
    assert_eq!(
        annotate
            .put_empty(&app.m(&format!("/test-cases/{tc}/tags/flaky")))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, case) = annotate.get(&app.m(&format!("/test-cases/{tc}"))).await;
    assert_eq!(case["tags"], json!(["flaky"]));

    // Annotations on a run, a test case and a point in time.
    let (status, on_run) = annotate
        .post(
            &app.m("/annotations"),
            &json!({"run_id": run_id, "body": "noisy"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    annotate
        .post(
            &app.m("/annotations"),
            &json!({"test_case_id": tc, "body": "new asset"}),
        )
        .await;
    annotate
        .post(
            &app.m("/annotations"),
            &json!({"occurred_at": "2026-11-15T00:00:00Z", "body": "switched to new compression"}),
        )
        .await;
    for bad in [
        json!({"body": "nowhere"}),
        json!({"run_id": run_id, "test_case_id": tc, "body": "two"}),
        json!({"run_id": run_id, "body": " "}),
    ] {
        assert_eq!(
            annotate.post(&app.m("/annotations"), &bad).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    let (_, run) = annotate.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["annotations"][0]["body"], "noisy");
    let (_, timed) = annotate
        .get(&app.m("/annotations?from=2026-11-01T00:00:00Z&to=2026-12-01T00:00:00Z"))
        .await;
    assert_eq!(timed.as_array().unwrap().len(), 1);
    let (_, by_case) = annotate
        .get(&app.m(&format!("/annotations?test_case_id={tc}")))
        .await;
    assert_eq!(by_case[0]["body"], "new asset");

    let id = on_run["id"].as_str().unwrap();
    let (status, edited) = annotate
        .patch(
            &app.m(&format!("/annotations/{id}")),
            &json!({"body": "very noisy"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edited["body"], "very noisy");
    assert_ne!(edited["updated_at"], on_run["updated_at"]);
    assert_eq!(
        annotate
            .delete(&app.m(&format!("/annotations/{id}")))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        annotate.get(&app.m(&format!("/annotations/{id}"))).await.0,
        StatusCode::NOT_FOUND
    );

    // The UI port can do the same without a token.
    let ui = app.ui_client();
    let (status, _) = ui
        .post(
            &app.u("/annotations"),
            &json!({"run_id": run_id, "body": "ui"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
}
