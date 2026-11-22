//! Uploaded metrics have one definition and no versions (FR-7, ADR 0018):
//! uploads create them, the web UI changes, renames and deletes them.

mod common;

use reqwest::StatusCode;
use serde_json::{Value, json};
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

async fn metric(api: &Api, app: &TestApp, name: &str) -> Value {
    let (status, metrics) = api.get(&app.m("/metrics")).await;
    assert_eq!(status, StatusCode::OK, "{metrics}");
    metrics
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == name)
        .cloned()
        .unwrap_or(Value::Null)
}

/// Uploads a case execution with one sample to a new run.
async fn upload(api: &Api, app: &TestApp, metrics: Value) -> (StatusCode, Value) {
    let run_id = Uuid::new_v4();
    let (status, body) = api
        .put(
            &app.m(&format!("/runs/{run_id}")),
            &run_body("2026-11-02T10:00:00Z"),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let mut ce = case_execution_body("pixel-8", 10, json!([sample(0, 2.0)]));
    ce["metrics"] = metrics;
    api.put(
        &app.m(&format!(
            "/runs/{run_id}/case-executions/{}",
            Uuid::new_v4()
        )),
        &ce,
    )
    .await
}

#[tokio::test]
async fn uploads_create_metrics_but_never_change_them() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let before = metric(&write, &app, "load_time").await;
    assert_eq!(
        (
            &before["unit"],
            &before["direction"],
            &before["description"]
        ),
        (&json!("s"), &json!("lower_is_better"), &json!(""))
    );

    // Another direction or description is ignored for a metric that exists.
    let mut metrics = metrics();
    metrics["load_time"]["direction"] = json!("higher_is_better");
    metrics["load_time"]["description"] = json!("time to first frame");
    let (status, out) = upload(&write, &app, metrics).await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    assert_eq!(metric(&write, &app, "load_time").await, before);

    // Another unit or value type is rejected: the values would be stored
    // under the wrong unit.
    for (field, value) in [("unit", "ms"), ("value_type", "integer")] {
        let mut metrics = common::metrics();
        metrics["load_time"][field] = json!(value);
        let (status, out) = upload(&write, &app, metrics).await;
        assert_eq!(
            (status, out["code"].as_str()),
            (StatusCode::CONFLICT, Some("metric_definition_conflict")),
            "{out}"
        );
    }
    assert_eq!(metric(&write, &app, "load_time").await, before);
}

#[tokio::test]
async fn metrics_are_changed_in_the_ui() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let read = app.as_scope(TokenScope::Read).await;
    let ui = app.ui_client();
    let (_, ce) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let (status, out) = ui
        .put(
            &app.u("/derived-metrics/padded"),
            &json!({"expression": "load_time + 1[s]", "unit": "s"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{out}");

    let save = |name: &str, body: Value| {
        let ui = ui.clone();
        let url = app.u(&format!("/metrics/{name}"));
        async move { ui.put(&url, &body).await }
    };
    let definition = |unit: &str| {
        json!({"unit": unit, "value_type": "float", "direction": "higher_is_better",
               "description": "Time to the first frame"})
    };

    // The change applies to the stored results: the values stay as they
    // are and are read in the new unit.
    let (status, out) = save("load_time", definition("ms")).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(
        (
            &out["unit"],
            &out["direction"],
            &out["description"],
            &out["has_results"],
            &out["used_by"]
        ),
        (
            &json!("ms"),
            &json!("higher_is_better"),
            &json!("Time to the first frame"),
            &json!(true),
            &json!(["padded"])
        )
    );
    assert!(out["updated_at"].as_str() > out["created_at"].as_str());
    assert_eq!(metric(&read, &app, "load_time").await, out);
    let (_, detail) = read.get(&app.m(&format!("/case-executions/{ce}"))).await;
    let measurement = &detail["samples"][0]["measurements"][0];
    assert_eq!(
        (
            &measurement["metric"],
            &measurement["unit"],
            &measurement["value"]
        ),
        (&json!("load_time"), &json!("ms"), &json!(1.5))
    );
    assert_eq!(measurement["metric_id"], out["id"]);
    // 1.5 ms + 1 s, in s.
    let (_, padded) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": ce, "name": "padded"}),
        )
        .await;
    assert_eq!(padded["samples"][0]["value"], json!(1.0015), "{padded}");

    // Uploads have to send the unit the metric has now.
    let mut metrics = metrics();
    let (status, out) = upload(&write, &app, metrics.clone()).await;
    assert_eq!(
        (status, out["code"].as_str()),
        (StatusCode::CONFLICT, Some("metric_definition_conflict")),
        "{out}"
    );
    metrics["load_time"]["unit"] = json!("ms");
    let (status, out) = upload(&write, &app, metrics).await;
    assert_eq!(status, StatusCode::CREATED, "{out}");

    // A unit that breaks a derived metric using the metric is rejected.
    let (status, out) = save("load_time", definition("MB")).await;
    assert_eq!(
        (status, out["code"].as_str()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Some("breaks_derived_metrics")
        ),
        "{out}"
    );
    assert!(out["detail"].as_str().unwrap().contains("derived.padded"));
    assert_eq!(metric(&read, &app, "load_time").await["unit"], "ms");

    assert_eq!(
        save("load_time", definition(" s")).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(save("none", definition("s")).await.0, StatusCode::NOT_FOUND);
    assert_eq!(metric(&read, &app, "none").await, Value::Null);

    // Not reachable with a token.
    let response = write
        .raw(
            write
                .client
                .put(app.m("/metrics/load_time"))
                .json(&definition("s")),
        )
        .await;
    assert!(!response.status().is_success(), "{}", response.status());
    assert_eq!(metric(&read, &app, "load_time").await["unit"], "ms");
}

#[tokio::test]
async fn metrics_rename_and_delete() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let read = app.as_scope(TokenScope::Read).await;
    let ui = app.ui_client();
    let (_, ce) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    for (name, body) in [
        (
            "double",
            json!({"expression": "load_time * 2", "unit": "s"}),
        ),
        (
            "peak",
            json!({"expression": "max(series.fps)", "unit": "fps"}),
        ),
        (
            "twice",
            json!({"expression": "frames * 2", "unit": "count"}),
        ),
    ] {
        let (status, out) = ui
            .put(&app.u(&format!("/derived-metrics/{name}")), &body)
            .await;
        assert_eq!(status, StatusCode::CREATED, "{out}");
    }
    let evaluate = |name: &str| {
        let read = read.clone();
        let url = app.m("/derived-metrics/evaluate");
        let body = json!({"case_execution_id": ce, "name": name});
        async move { read.post(&url, &body).await.1["samples"][0]["value"].clone() }
    };
    let (double, peak) = (evaluate("double").await, evaluate("peak").await);
    assert_eq!(double, json!(3.0));

    // Renaming keeps the results and rewrites the expressions that use the
    // metric, as scalar or as series.
    let rename = |name: &str, new: &str| {
        let ui = ui.clone();
        let url = app.u(&format!("/metrics/{name}/rename"));
        let body = json!({"name": new});
        async move { ui.post(&url, &body).await }
    };
    let (status, out) = rename("load_time", "load time").await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(
        (out["name"].as_str(), &out["rewritten"], &out["used_by"]),
        (Some("load time"), &json!(["double"]), &json!(["double"]))
    );
    let (status, out) = rename("fps", "frame_rate").await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["rewritten"], json!(["peak"]));
    let expression = |name: &str| {
        let read = read.clone();
        let url = app.m(&format!("/derived-metrics/{name}"));
        async move { read.get(&url).await.1["expression"].clone() }
    };
    assert_eq!(expression("double").await, "`load time` * 2");
    assert_eq!(expression("peak").await, "max(series.frame_rate)");
    assert_eq!(expression("twice").await, "frames * 2");
    assert_eq!(evaluate("double").await, double);
    assert_eq!(evaluate("peak").await, peak);
    let (_, detail) = read.get(&app.m(&format!("/case-executions/{ce}"))).await;
    assert_eq!(
        (
            &detail["samples"][0]["measurements"][0]["metric"],
            &detail["samples"][0]["series"][0]["metric"]
        ),
        (&json!("load time"), &json!("frame_rate"))
    );
    assert_eq!(metric(&read, &app, "load_time").await, Value::Null);

    // The same name is a no-op; a taken, invalid or missing one is not.
    assert_eq!(rename("frames", "frames").await.0, StatusCode::OK);
    let (status, out) = rename("frames", "frame_rate").await;
    assert_eq!(
        (status, out["code"].as_str()),
        (StatusCode::CONFLICT, Some("duplicate"))
    );
    assert_eq!(
        rename("frames", "a`b").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(rename("none", "x").await.0, StatusCode::NOT_FOUND);

    // An upload under the old name creates a new metric.
    let (status, out) = upload(&write, &app, metrics()).await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    assert_eq!(metric(&read, &app, "load_time").await["used_by"], json!([]));
    assert_eq!(
        metric(&read, &app, "load time").await["has_results"],
        json!(true)
    );

    // A metric with results, or one that derived metrics use, cannot be
    // deleted.
    let delete = |name: &str| {
        let ui = ui.clone();
        let url = app.u(&format!("/metrics/{name}"));
        async move { ui.delete(&url).await }
    };
    let frames = metric(&read, &app, "frames").await;
    assert_eq!(
        (&frames["has_results"], &frames["used_by"]),
        (&json!(false), &json!(["twice"]))
    );
    let (status, out) = delete("frames").await;
    assert_eq!(
        (status, out["code"].as_str()),
        (StatusCode::CONFLICT, Some("metric_in_use")),
        "{out}"
    );
    assert!(out["detail"].as_str().unwrap().contains("twice"), "{out}");
    let (status, out) = delete("load time").await;
    assert_eq!(
        (status, out["code"].as_str()),
        (StatusCode::CONFLICT, Some("metric_in_use")),
        "{out}"
    );
    assert_eq!(
        ui.delete(&app.u("/derived-metrics/twice")).await.0,
        StatusCode::NO_CONTENT
    );

    // Neither is reachable with a token.
    for request in [
        write.client.delete(app.m("/metrics/frames")),
        write
            .client
            .post(app.m("/metrics/frames/rename"))
            .json(&json!({"name": "x"})),
    ] {
        let response = write.raw(request).await;
        assert!(!response.status().is_success(), "{}", response.status());
    }
    assert_eq!(metric(&read, &app, "frames").await["name"], "frames");

    assert_eq!(delete("frames").await.0, StatusCode::NO_CONTENT);
    assert_eq!(delete("frames").await.0, StatusCode::NOT_FOUND);
    assert_eq!(metric(&read, &app, "frames").await, Value::Null);
}
