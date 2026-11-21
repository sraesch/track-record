//! Integration test harness: a fresh database per test (created from the
//! migrated `track_record` database as a template) and the app serving both
//! listeners on ephemeral ports.
//!
//! Needs `TEST_DATABASE_URL` pointing at the `postgres` database of a server
//! started from the track-record-db image (backend/test.sh does that).
//! Without it, tests are skipped with a notice, unless `TR_REQUIRE_DB=1`.

#![allow(dead_code)]

use std::sync::Arc;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use track_record_server::AppState;
use track_record_server::attachment_store::AttachmentStore;
use track_record_server::db::TokenScope;
use track_record_server::service::tokens::{self, CreateToken};
use uuid::Uuid;

pub struct TestApp {
    pub machine: String,
    pub ui: String,
    pub client: Client,
    pub pool: PgPool,
    pub state: AppState,
    pub attachment_dir: TempDir,
    cancel: CancellationToken,
    admin_url: String,
    db_name: String,
}

/// Skips the test (returns) when no database is configured.
#[macro_export]
macro_rules! app {
    () => {
        match common::TestApp::spawn().await {
            Some(app) => app,
            None => return,
        }
    };
}

fn with_database(url: &str, db: &str) -> String {
    let (base, _) = url.rsplit_once('/').expect("database URL has a path");
    format!("{base}/{db}")
}

impl TestApp {
    pub async fn spawn() -> Option<Self> {
        let Ok(admin_url) = std::env::var("TEST_DATABASE_URL") else {
            if std::env::var("TR_REQUIRE_DB").as_deref() == Ok("1") {
                panic!("TEST_DATABASE_URL is not set but TR_REQUIRE_DB=1");
            }
            eprintln!("skipping: TEST_DATABASE_URL is not set (run backend/test.sh)");
            return None;
        };
        let db_name = format!("t_{}", Uuid::new_v4().simple());
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&admin_url)
            .await
            .unwrap();
        // Template copies fail while another session is connected to the
        // template; retry briefly.
        let mut attempt = 0;
        loop {
            let created = sqlx::query(&format!("CREATE DATABASE {db_name} TEMPLATE track_record"))
                .execute(&admin)
                .await;
            match created {
                Ok(_) => break,
                Err(e) if attempt < 50 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if attempt == 50 {
                        panic!("cannot create test database: {e}");
                    }
                }
                Err(e) => panic!("cannot create test database: {e}"),
            }
        }
        admin.close().await;

        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&with_database(&admin_url, &db_name))
            .await
            .unwrap();
        track_record_server::db::check_schema(&pool).await.unwrap();

        let attachment_dir = tempfile::tempdir().unwrap();
        let state = AppState {
            pool: pool.clone(),
            attachments: Arc::new(AttachmentStore::new(attachment_dir.path())),
            max_body_bytes: 64 * 1024 * 1024,
        };
        state.attachments.init().await.unwrap();

        let machine = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ui = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let machine_url = format!("http://{}", machine.local_addr().unwrap());
        let ui_url = format!("http://{}", ui.local_addr().unwrap());
        let cancel = CancellationToken::new();
        tokio::spawn(track_record_server::serve(
            state.clone(),
            machine,
            ui,
            cancel.clone(),
        ));

        Some(Self {
            machine: machine_url,
            ui: ui_url,
            client: Client::new(),
            pool,
            state,
            attachment_dir,
            cancel,
            admin_url,
            db_name,
        })
    }

    /// URL of a machine API path, e.g. `m("/runs")`.
    pub fn m(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.machine)
    }

    /// URL of a UI API path.
    pub fn u(&self, path: &str) -> String {
        format!("{}/ui/api/v1{path}", self.ui)
    }

    pub async fn token(&self, scope: TokenScope) -> String {
        tokens::create(
            &self.pool,
            &CreateToken {
                name: format!("{scope:?}"),
                scope,
            },
        )
        .await
        .unwrap()
        .secret
    }

    /// A client bound to a token.
    pub async fn as_scope(&self, scope: TokenScope) -> Api {
        Api {
            client: self.client.clone(),
            token: Some(self.token(scope).await),
        }
    }

    /// Client for the UI port (no token).
    pub fn ui_client(&self) -> Api {
        Api {
            client: self.client.clone(),
            token: None,
        }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.cancel.cancel();
        let (admin_url, db_name) = (self.admin_url.clone(), self.db_name.clone());
        // Best effort; the container is thrown away anyway.
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                if let Ok(admin) = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&admin_url)
                    .await
                {
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
                        .execute(&admin)
                        .await;
                }
            });
        });
    }
}

#[derive(Clone)]
pub struct Api {
    pub client: Client,
    pub token: Option<String>,
}

impl Api {
    fn auth(&self, rb: RequestBuilder) -> RequestBuilder {
        match &self.token {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        }
    }
    pub async fn get(&self, url: &str) -> (StatusCode, Value) {
        send(self.auth(self.client.get(url))).await
    }
    pub async fn put(&self, url: &str, body: &Value) -> (StatusCode, Value) {
        send(self.auth(self.client.put(url).json(body))).await
    }
    pub async fn put_empty(&self, url: &str) -> (StatusCode, Value) {
        send(self.auth(self.client.put(url))).await
    }
    pub async fn post(&self, url: &str, body: &Value) -> (StatusCode, Value) {
        send(self.auth(self.client.post(url).json(body))).await
    }
    pub async fn patch(&self, url: &str, body: &Value) -> (StatusCode, Value) {
        send(self.auth(self.client.patch(url).json(body))).await
    }
    pub async fn delete(&self, url: &str) -> (StatusCode, Value) {
        send(self.auth(self.client.delete(url))).await
    }
    pub async fn raw(&self, rb: RequestBuilder) -> Response {
        self.auth(rb).send().await.unwrap()
    }
}

pub async fn send(rb: RequestBuilder) -> (StatusCode, Value) {
    let response = rb.send().await.unwrap();
    let status = response.status();
    let bytes = response.bytes().await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, value)
}

// Request bodies ------------------------------------------------------------------

pub fn run_body(started_at: &str) -> Value {
    json!({
        "suite": {"name": "nightly", "definition": {"scenarios": ["load-model"]}},
        "sut_version": {"commit": "abc123", "branch": "main", "build_config": "release"},
        "shared_environment": {"server": "backend-1"},
        "triggered_by": "nightly",
        "started_at": started_at,
        "tags": ["nightly"]
    })
}

pub fn metrics() -> Value {
    json!({
        "load_time": {"unit": "s", "value_type": "float", "direction": "lower_is_better"},
        "fps": {"unit": "fps", "value_type": "float", "direction": "higher_is_better"},
        "frames": {"unit": "count", "value_type": "integer", "direction": "higher_is_better"}
    })
}

pub fn case_execution_body(device: &str, size_mb: i64, samples: Value) -> Value {
    json!({
        "test_case": {
            "scenario": {"name": "load-model", "definition": {"steps": ["open", "render"]}},
            "assets": [{"name": "engine.glb", "version": "v1"}],
            "parameters": {"size_mb": size_mb, "pipeline": "classic"},
            "parameter_units": {"size_mb": "MB"}
        },
        "client_device": {"name": device, "details": {"os": "Android 15"}},
        "metrics": metrics(),
        "samples": samples
    })
}

pub fn sample(repetition: i64, load_time: f64) -> Value {
    json!({
        "id": Uuid::new_v4(),
        "repetition": repetition,
        "measurements": {"load_time": load_time},
        "series": [{
            "id": Uuid::new_v4(),
            "metric": "fps",
            "x_dimension": "time",
            "x_unit": "s",
            "x": [0.0, 0.5, 1.0, 1.0, 2.0],
            "y": [10.0, 30.0, 60.0, 55.0, 58.0]
        }]
    })
}

/// Creates a run with one case execution and two samples; returns
/// (run_id, case_execution_id).
pub async fn seed_run(api: &Api, app: &TestApp, device: &str, started_at: &str) -> (Uuid, Uuid) {
    let run_id = Uuid::new_v4();
    let (status, body) = api
        .put(&app.m(&format!("/runs/{run_id}")), &run_body(started_at))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let ce_id = Uuid::new_v4();
    let (status, body) = api
        .put(
            &app.m(&format!("/runs/{run_id}/case-executions/{ce_id}")),
            &case_execution_body(device, 10, json!([sample(0, 1.5), sample(1, 1.25)])),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (run_id, ce_id)
}
