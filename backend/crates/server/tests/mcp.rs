//! MCP server on the machine port: tools per token scope, read tools give the
//! same results as REST, tag and annotation tools write through the same
//! services. No upload, invalidation or deletion tools exist.

mod common;

use reqwest::StatusCode;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

type Client = RunningService<RoleClient, ()>;

async fn connect(app: &TestApp, token: &str) -> Client {
    let config = StreamableHttpClientTransportConfig::with_uri(format!("{}/mcp", app.machine))
        .auth_header(token);
    ().serve(StreamableHttpClientTransport::from_config(config))
        .await
        .expect("MCP initialize")
}

async fn mcp(app: &TestApp, scope: TokenScope) -> Client {
    connect(app, &app.token(scope).await).await
}

async fn tool_names(client: &Client) -> Vec<String> {
    client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.into_owned())
        .collect()
}

async fn call_raw(client: &Client, name: &str, args: Value) -> CallToolResult {
    let args = args.as_object().expect("arguments are an object").clone();
    client
        .call_tool(CallToolRequestParams::new(name.to_owned()).with_arguments(args))
        .await
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Calls a tool that must succeed; returns its structured content.
async fn call(client: &Client, name: &str, args: Value) -> Value {
    let result = call_raw(client, name, args).await;
    let content = result.structured_content.clone().unwrap_or(Value::Null);
    assert_ne!(result.is_error, Some(true), "{name} failed: {content}");
    content
}

/// Calls a tool that must fail with a tool error; returns its content.
async fn call_err(client: &Client, name: &str, args: Value) -> Value {
    let result = call_raw(client, name, args).await;
    assert_eq!(result.is_error, Some(true), "{name} should fail");
    result.structured_content.unwrap()
}

const READ_TOOLS: &[&str] = &[
    "aggregates",
    "compare",
    "evaluate_derived_metric",
    "get_case_execution",
    "get_run",
    "list_annotations",
    "list_case_executions",
    "list_client_devices",
    "list_derived_metrics",
    "list_metrics",
    "list_runs",
    "list_scenarios",
    "list_suites",
    "list_test_cases",
    "trend",
    "validate_derived_metric",
];

const ANNOTATE_TOOLS: &[&str] = &[
    "add_run_tag",
    "add_test_case_tag",
    "create_annotation",
    "delete_annotation",
    "remove_run_tag",
    "remove_test_case_tag",
    "update_annotation",
];

#[tokio::test]
async fn tools_depend_on_the_token_scope() {
    let app = app!();

    let mut expected: Vec<&str> = READ_TOOLS.to_vec();
    expected.sort_unstable();
    for scope in [TokenScope::Read, TokenScope::Write] {
        let client = mcp(&app, scope).await;
        assert_eq!(tool_names(&client).await, expected, "{scope:?}");
    }

    let client = mcp(&app, TokenScope::Annotate).await;
    let mut expected: Vec<&str> = READ_TOOLS.iter().chain(ANNOTATE_TOOLS).copied().collect();
    expected.sort_unstable();
    let names = tool_names(&client).await;
    assert_eq!(names, expected);
    // MCP is read + tags/annotations only.
    for forbidden in [
        "upload",
        "invalidate",
        "validate_run",
        "delete_run",
        "token",
        "alias",
    ] {
        assert!(
            !names.iter().any(|n| n.contains(forbidden)),
            "{forbidden} in {names:?}"
        );
    }

    // Read tools are marked read-only and have an object input schema.
    let tools = client.list_all_tools().await.unwrap();
    let list_runs = tools.iter().find(|t| t.name == "list_runs").unwrap();
    assert_eq!(
        list_runs.annotations.as_ref().unwrap().read_only_hint,
        Some(true)
    );
    assert_eq!(list_runs.input_schema["type"], "object");
    assert!(list_runs.input_schema["properties"]["client_device"].is_object());
}

#[tokio::test]
async fn annotate_tools_are_refused_without_annotate_scope() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let (run_id, _) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;

    for scope in [TokenScope::Read, TokenScope::Write] {
        let client = mcp(&app, scope).await;
        let args = json!({"run_id": run_id, "tag": "x"})
            .as_object()
            .unwrap()
            .clone();
        let err = client
            .call_tool(CallToolRequestParams::new("add_run_tag").with_arguments(args))
            .await
            .expect_err("annotate tool with a {scope:?} token");
        assert!(err.to_string().contains("not found"), "{err}");
    }
    let (_, run) = write.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["tags"], json!(["nightly"]));
}

#[tokio::test]
async fn mcp_needs_a_valid_token() {
    let app = app!();
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "test", "version": "0"}}
    });
    let post = || {
        app.client
            .post(format!("{}/mcp", app.machine))
            .header("Accept", "application/json, text/event-stream")
            .json(&init)
    };

    let (status, body) = send(post()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthorized");

    let (status, _) = send(post().bearer_auth("trk_wrong")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let token = app.token(TokenScope::Read).await;
    let (status, body) = send(post().bearer_auth(&token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["result"]["serverInfo"]["name"], "track-record");

    // Revoked tokens stop working.
    let ui = app.ui_client();
    let (_, tokens) = ui.get(&app.u("/tokens")).await;
    let id = tokens.as_array().unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = ui
        .post(&app.u(&format!("/tokens/{id}/revoke")), &json!({}))
        .await;
    assert!(status.is_success(), "{status}");
    let (status, _) = send(post().bearer_auth(&token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The MCP server is not on the UI port.
    let (status, _) = send(
        app.client
            .post(format!("{}/mcp", app.ui))
            .header("Accept", "application/json, text/event-stream")
            .json(&init),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn read_tools_return_what_rest_returns() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let read = app.as_scope(TokenScope::Read).await;
    let (run_a, ce_a) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let (run_b, _) = seed_run(&write, &app, "iphone-16", "2026-11-02T10:00:00Z").await;
    let client = mcp(&app, TokenScope::Read).await;

    let runs = call(&client, "list_runs", json!({"client_device": "pixel-8"})).await;
    let (_, rest) = read.get(&app.m("/runs?client_device=pixel-8")).await;
    assert_eq!(runs, rest);
    assert_eq!(runs["runs"][0]["id"], json!(run_a));
    let runs = call(&client, "list_runs", json!({"latest_runs": 1})).await;
    let (_, rest) = read.get(&app.m("/runs?latest_runs=1")).await;
    assert_eq!(runs, rest);
    assert_eq!(runs["runs"].as_array().unwrap().len(), 1);
    assert_eq!(runs["runs"][0]["id"], json!(run_b));
    let runs = call(&client, "list_runs", json!({"since_tag": "nightly"})).await;
    assert_eq!(runs["runs"].as_array().unwrap().len(), 2);
    let runs = call(&client, "list_runs", json!({"started_within_days": 1})).await;
    assert_eq!(
        runs["runs"],
        json!([]),
        "the seeded runs started on 2026-11-01 and 02"
    );

    let run = call(&client, "get_run", json!({"run_id": run_b})).await;
    let (_, rest) = read.get(&app.m(&format!("/runs/{run_b}"))).await;
    assert_eq!(run, rest);

    let ces = call(&client, "list_case_executions", json!({"run_id": run_a})).await;
    assert_eq!(ces["items"][0]["id"], json!(ce_a));
    let ce = call(
        &client,
        "get_case_execution",
        json!({"case_execution_id": ce_a}),
    )
    .await;
    let (_, rest) = read.get(&app.m(&format!("/case-executions/{ce_a}"))).await;
    assert_eq!(ce, rest);

    // Lists are wrapped in `items`.
    let devices = call(&client, "list_client_devices", json!({})).await;
    let (_, rest) = read.get(&app.m("/client-devices")).await;
    assert_eq!(devices["items"], rest);
    let metrics = call(&client, "list_metrics", json!({})).await;
    assert_eq!(metrics["items"].as_array().unwrap().len(), 3);

    let req = json!({"metric": "load_time", "unit": "ms"});
    let aggregates = call(&client, "aggregates", req.clone()).await;
    let (status, rest) = read.post(&app.m("/analysis/aggregates"), &req).await;
    assert_eq!(status, StatusCode::OK, "{rest}");
    assert_eq!(aggregates, rest);

    let test_case_id = ce["test_case"]["id"].clone();
    let req = json!({"test_case_id": test_case_id, "client_device": "pixel-8",
                     "metric": "load_time"});
    let trend = call(&client, "trend", req.clone()).await;
    let (status, rest) = read.post(&app.m("/analysis/trend"), &req).await;
    assert_eq!(status, StatusCode::OK, "{rest}");
    assert_eq!(trend, rest);

    // The comparison stream collected into one result.
    let compare = call(
        &client,
        "compare",
        json!({"metric": "load_time", "dimension": "client_device",
               "values": ["pixel-8", "iphone-16"]}),
    )
    .await;
    assert_eq!(compare["dimension"], "client_device");
    assert_eq!(compare["reference"], "pixel-8");
    let rows = compare["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{compare}");
    let cells = rows[0]["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0]["is_reference"], true);
    // Same SUT version and environment: nothing is marked.
    assert_eq!(cells[1]["sut_version_differs"], false);
    assert!(compare["excluded"].is_object());

    // Derived metric DSL is the same as in the API.
    let ok = call(
        &client,
        "validate_derived_metric",
        json!({"expression": "load_time * 1000", "unit": "ms"}),
    )
    .await;
    assert_eq!(ok["valid"], true, "{ok}");
    let bad = call(
        &client,
        "validate_derived_metric",
        json!({"expression": "load_time +"}),
    )
    .await;
    assert_eq!(bad["valid"], false);
    let eval = call(
        &client,
        "evaluate_derived_metric",
        json!({"case_execution_id": ce_a, "expression": "max(series.fps)"}),
    )
    .await;
    assert_eq!(eval["samples"].as_array().unwrap().len(), 2, "{eval}");
}

#[tokio::test]
async fn tool_errors_carry_the_api_error_code() {
    let app = app!();
    let client = mcp(&app, TokenScope::Read).await;

    let err = call_err(&client, "get_run", json!({"run_id": Uuid::new_v4()})).await;
    assert_eq!(err["code"], "not_found");
    assert_eq!(err["status"], 404);

    let err = call_err(&client, "list_runs", json!({"since_sut_tag": "nope"})).await;
    assert_eq!(
        (err["code"].as_str(), err["status"].as_i64()),
        (Some("validation_failed"), Some(422))
    );

    let err = call_err(&client, "aggregates", json!({"metric": "nope"})).await;
    assert_eq!(err["status"], 422, "{err}");

    let err = call_err(
        &client,
        "compare",
        json!({"metric": "load_time", "dimension": "parameters"}),
    )
    .await;
    assert_eq!(err["code"], "bad_request");

    // Arguments are checked against the schema; a mismatch is a tool error.
    let result = call_raw(&client, "get_run", json!({"run_id": "not-a-uuid"})).await;
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn annotate_tools_write_tags_and_annotations() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let read = app.as_scope(TokenScope::Read).await;
    let (run_id, ce_id) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let client = mcp(&app, TokenScope::Annotate).await;

    call(
        &client,
        "add_run_tag",
        json!({"run_id": run_id, "tag": "regression"}),
    )
    .await;
    call(
        &client,
        "add_run_tag",
        json!({"run_id": run_id, "tag": "regression"}),
    )
    .await;
    let (_, run) = read.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["tags"], json!(["nightly", "regression"]));
    call(
        &client,
        "remove_run_tag",
        json!({"run_id": run_id, "tag": "nightly"}),
    )
    .await;
    let (_, run) = read.get(&app.m(&format!("/runs/{run_id}"))).await;
    assert_eq!(run["tags"], json!(["regression"]));

    let (_, ce) = read.get(&app.m(&format!("/case-executions/{ce_id}"))).await;
    let test_case_id = ce["test_case"]["id"].as_i64().unwrap();
    call(
        &client,
        "add_test_case_tag",
        json!({"test_case_id": test_case_id, "tag": "flaky"}),
    )
    .await;
    let cases = call(
        &client,
        "list_test_cases",
        json!({"scenario": "load-model"}),
    )
    .await;
    assert_eq!(cases["items"][0]["tags"], json!(["flaky"]));
    call(
        &client,
        "remove_test_case_tag",
        json!({"test_case_id": test_case_id, "tag": "flaky"}),
    )
    .await;
    let cases = call(&client, "list_test_cases", json!({})).await;
    assert_eq!(cases["items"][0]["tags"], json!([]));

    let created = call(
        &client,
        "create_annotation",
        json!({"run_id": run_id, "body": "slow because of thermal throttling"}),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_owned();
    let listed = call(&client, "list_annotations", json!({"run_id": run_id})).await;
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);

    let updated = call(
        &client,
        "update_annotation",
        json!({"annotation_id": id, "body": "thermal throttling"}),
    )
    .await;
    assert_eq!(updated["body"], "thermal throttling");
    let (_, rest) = read.get(&app.m(&format!("/annotations/{id}"))).await;
    assert_eq!(rest["body"], "thermal throttling");

    let err = call_err(
        &client,
        "create_annotation",
        json!({"run_id": run_id, "body": "  "}),
    )
    .await;
    assert_eq!(err["code"], "validation_failed");

    call(&client, "delete_annotation", json!({"annotation_id": id})).await;
    let (status, _) = read.get(&app.m(&format!("/annotations/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
