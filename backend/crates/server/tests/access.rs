//! Access control (NFR-2): token scopes on the machine port, UI-only
//! endpoints unreachable with any token.

mod common;

use reqwest::StatusCode;
use serde_json::json;
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn machine_port_requires_valid_tokens() {
    let app = app!();
    let anonymous = Api {
        client: app.client.clone(),
        token: None,
    };
    let (status, problem) = anonymous.get(&app.m("/runs")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(problem["code"], "unauthorized");

    let forged = Api {
        client: app.client.clone(),
        token: Some("trk_forged".into()),
    };
    assert_eq!(
        forged.get(&app.m("/runs")).await.0,
        StatusCode::UNAUTHORIZED
    );

    // Health and the OpenAPI document need no token.
    assert_eq!(
        anonymous.get(&format!("{}/healthz", app.machine)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        anonymous.get(&format!("{}/readyz", app.machine)).await.0,
        StatusCode::OK
    );
    let (status, doc) = anonymous
        .get(&format!("{}/api/openapi.json", app.machine))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(doc["paths"]["/api/v1/runs/{run_id}"]["put"].is_object());
}

#[tokio::test]
async fn scopes_limit_what_a_token_can_do() {
    let app = app!();
    let read = app.as_scope(TokenScope::Read).await;
    let annotate = app.as_scope(TokenScope::Annotate).await;
    let write = app.as_scope(TokenScope::Write).await;

    // Everyone can read.
    for api in [&read, &annotate, &write] {
        assert_eq!(api.get(&app.m("/runs")).await.0, StatusCode::OK);
    }

    // Only write can ingest.
    let body = run_body("2026-11-20T22:00:00Z");
    for api in [&read, &annotate] {
        let (status, problem) = api
            .put(&app.m(&format!("/runs/{}", Uuid::new_v4())), &body)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
    }
    let (run_id, _) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;

    // Only annotate can tag and annotate.
    let tag_url = app.m(&format!("/runs/{run_id}/tags/regression"));
    assert_eq!(read.put_empty(&tag_url).await.0, StatusCode::FORBIDDEN);
    assert_eq!(write.put_empty(&tag_url).await.0, StatusCode::FORBIDDEN);
    assert_eq!(annotate.put_empty(&tag_url).await.0, StatusCode::NO_CONTENT);
    let note = json!({"run_id": run_id, "body": "slow network"});
    assert_eq!(
        write.post(&app.m("/annotations"), &note).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        annotate.post(&app.m("/annotations"), &note).await.0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn revoked_tokens_are_rejected() {
    let app = app!();
    let ui = app.ui_client();
    let (status, created) = ui
        .post(
            &app.u("/tokens"),
            &json!({"name": "harness", "scope": "write"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("trk_"));
    let api = Api {
        client: app.client.clone(),
        token: Some(secret.clone()),
    };
    assert_eq!(api.get(&app.m("/runs")).await.0, StatusCode::OK);

    // The secret is never listed, but last use is.
    let (_, list) = ui.get(&app.u("/tokens")).await;
    assert!(!list.to_string().contains(&secret));
    assert!(list[0]["last_used_at"].is_string());

    let id = created["id"].as_str().unwrap();
    let (status, revoked) = ui
        .post(&app.u(&format!("/tokens/{id}/revoke")), &json!(null))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(revoked["revoked_at"].is_string());
    assert_eq!(api.get(&app.m("/runs")).await.0, StatusCode::UNAUTHORIZED);

    // Only the hash is stored.
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT token_hash FROM api_token WHERE id = $1::uuid")
            .bind(id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(stored, track_record_server::auth::hash_token(&secret));
}

#[tokio::test]
async fn ui_only_endpoints_do_not_exist_on_the_machine_port() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let annotate = app.as_scope(TokenScope::Annotate).await;
    let (run_id, _) = seed_run(&write, &app, "dev", "2026-11-20T22:00:00Z").await;

    for api in [&write, &annotate] {
        let calls = [
            api.post(
                &app.m(&format!("/runs/{run_id}/invalidate")),
                &json!({"reason": "x"}),
            )
            .await,
            api.post(&app.m(&format!("/runs/{run_id}/validate")), &json!(null))
                .await,
            api.post(
                &app.m(&format!("/runs/{run_id}/delete")),
                &json!({"confirm_run_id": run_id}),
            )
            .await,
            api.delete(&app.m(&format!("/runs/{run_id}"))).await,
            api.get(&app.m("/tokens")).await,
            api.post(&app.m("/tokens"), &json!({"name": "x", "scope": "write"}))
                .await,
            api.put(
                &app.m("/aliases"),
                &json!({"dimension": "client_device", "source": "a", "target": "b"}),
            )
            .await,
            // The UI prefix is not routed on the machine port either.
            api.get(&format!("{}/ui/api/v1/tokens", app.machine)).await,
        ];
        // Not routed (404/405), or stopped by the scope check of a
        // neighbouring route on the same path (403): never executed.
        for (status, _) in calls {
            assert!(
                [
                    StatusCode::NOT_FOUND,
                    StatusCode::METHOD_NOT_ALLOWED,
                    StatusCode::FORBIDDEN
                ]
                .contains(&status),
                "got {status}"
            );
        }
    }
    // The run is untouched.
    let (_, run) = write.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["is_valid"], true);

    // And the UI port has no ingestion.
    let ui = app.ui_client();
    let (status, _) = ui
        .put(
            &app.u(&format!("/runs/{}", Uuid::new_v4())),
            &run_body("2026-11-20T22:00:00Z"),
        )
        .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}
