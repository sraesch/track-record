//! Analysis: aggregates (FR-12), trends (FR-18), comparisons (FR-16), series
//! views (FR-19) and export (FR-20).

mod common;

use reqwest::StatusCode;
use serde_json::{Value, json};
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

fn s(repetition: i64, load_time: f64, warmup: bool) -> Value {
    let mut v = sample(repetition, load_time);
    v["is_warmup"] = json!(warmup);
    v
}

async fn new_run(api: &Api, app: &TestApp, started_at: &str, commit: &str) -> Uuid {
    let id = Uuid::new_v4();
    let mut body = run_body(started_at);
    body["sut_version"]["commit"] = json!(commit);
    let (status, b) = api.put(&app.m(&format!("/runs/{id}")), &body).await;
    assert_eq!(status, StatusCode::CREATED, "{b}");
    id
}

struct Ce<'a> {
    device: &'a str,
    os: &'a str,
    pipeline: &'a str,
    size_mb: i64,
}

impl Default for Ce<'_> {
    fn default() -> Self {
        Self {
            device: "pixel-8",
            os: "Android 15",
            pipeline: "classic",
            size_mb: 10,
        }
    }
}

async fn add_ce(api: &Api, app: &TestApp, run: Uuid, ce: Ce<'_>, samples: Value) -> Uuid {
    let id = Uuid::new_v4();
    let mut body = case_execution_body(ce.device, ce.size_mb, samples);
    body["client_device"]["details"]["os"] = json!(ce.os);
    body["test_case"]["parameters"]["pipeline"] = json!(ce.pipeline);
    let (status, b) = api
        .put(&app.m(&format!("/runs/{run}/case-executions/{id}")), &body)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{b}");
    id
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64()
        .is_some_and(|a| (a - b).abs() < 1e-9 * b.abs().max(1.0))
}

#[tokio::test]
async fn aggregates_exclude_warmup_and_invalid_runs() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let a = new_run(&write, &app, "2026-11-01T10:00:00Z", "abc").await;
    add_ce(
        &write,
        &app,
        a,
        Ce::default(),
        json!([
            s(0, 100.0, true),
            s(1, 1.0, false),
            s(2, 2.0, false),
            s(3, 3.0, false)
        ]),
    )
    .await;
    let b = new_run(&write, &app, "2026-11-02T10:00:00Z", "abc").await;
    add_ce(
        &write,
        &app,
        b,
        Ce::default(),
        json!([s(0, 4.0, false), s(1, 6.0, false)]),
    )
    .await;
    let (status, _) = ui
        .post(
            &app.u(&format!("/runs/{b}/invalidate")),
            &json!({"reason": "broken"}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let agg = |body: Value| {
        let ui = ui.clone();
        let url = app.u("/analysis/aggregates");
        async move {
            let (status, out) = ui.post(&url, &body).await;
            (status, out)
        }
    };

    let (status, out) = agg(json!({"metric": "load_time"})).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["metric"]["unit"], "s");
    let groups = out["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "invalid run excluded: {out}");
    let st = &groups[0]["stats"];
    assert_eq!(groups[0]["run"]["id"], a.to_string());
    assert_eq!(groups[0]["client_device"], "pixel-8");
    assert_eq!(groups[0]["case"]["parameters"]["pipeline"], "classic");
    assert_eq!(
        (st["n"].as_u64(), st["n_no_value"].as_u64()),
        (Some(3), Some(0))
    );
    assert!(close(&st["mean"], 2.0) && close(&st["median"], 2.0) && close(&st["stddev"], 1.0));
    assert!(close(&st["min"], 1.0) && close(&st["max"], 3.0));
    let half = 4.302_652_729_911_275 / 3f64.sqrt();
    assert!(
        close(&st["ci_low"], 2.0 - half) && close(&st["ci_high"], 2.0 + half),
        "{st}"
    );
    assert_eq!(st["percentiles"].as_array().unwrap().len(), 4);

    let (_, out) = agg(json!({"metric": "load_time", "include_warmup": true})).await;
    assert_eq!(out["groups"][0]["stats"]["n"], 4);
    assert!(close(&out["groups"][0]["stats"]["mean"], 26.5));

    let (_, out) = agg(json!({"metric": "load_time", "filter": {"validity": "all"}})).await;
    assert_eq!(out["groups"].as_array().unwrap().len(), 2);
    assert!(close(&out["groups"][1]["stats"]["mean"], 5.0));
    assert_eq!(out["groups"][1]["run"]["is_valid"], false);

    let (_, out) = agg(json!({"metric": "load_time", "unit": "ms", "percentiles": [50]})).await;
    assert!(close(&out["groups"][0]["stats"]["mean"], 2000.0));
    assert_eq!(out["metric"]["unit"], "ms");
    assert!(close(
        &out["groups"][0]["stats"]["percentiles"][0]["value"],
        2000.0
    ));

    let (_, out) = agg(json!({"metric": "load_time * 2"})).await;
    assert!(close(&out["groups"][0]["stats"]["mean"], 4.0));

    // A metric the samples do not have: excluded as missing, not no value.
    let (_, out) = agg(json!({"metric": "frames"})).await;
    assert_eq!(out["groups"].as_array().unwrap().len(), 0);
    assert_eq!(out["excluded"]["missing"], 3);

    let (status, out) = agg(json!({"metric": "series.fps"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{out}");
    let (status, out) = agg(json!({"metric": "nope +"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(out["code"], "invalid_expression");
    let (status, _) = agg(json!({"metric": "load_time", "unit": "B"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = agg(json!({"metric": "load_time", "filter": {"limit": 5}})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = agg(json!({"metric": "load_time", "bogus": 1})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Parameter aliases apply (FR-14) and keep the trend key.
    let (status, _) = ui
        .put(
            &app.u("/aliases"),
            &json!({"dimension": "parameter:pipeline", "source": "classic", "target": "legacy"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, out) =
        agg(json!({"metric": "load_time", "filter": {"param": ["pipeline=legacy"]}})).await;
    assert_eq!(out["groups"].as_array().unwrap().len(), 1, "{out}");
    assert_eq!(out["groups"][0]["case"]["parameters"]["pipeline"], "legacy");

    // Same endpoint on the machine port with a read token.
    let read = app.as_scope(TokenScope::Read).await;
    let (status, out) = read
        .post(
            &app.m("/analysis/aggregates"),
            &json!({"metric": "load_time"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    let (status, _) = app
        .ui_client()
        .post(
            &app.m("/analysis/aggregates"),
            &json!({"metric": "load_time"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn derived_metrics_validate_save_and_evaluate() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let read = app.as_scope(TokenScope::Read).await;
    let ui = app.ui_client();
    let (_, ce) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;

    let validate = |body: Value| {
        let read = read.clone();
        let url = app.m("/derived-metrics/validate");
        async move {
            let (status, out) = read.post(&url, &body).await;
            assert_eq!(status, StatusCode::OK, "{out}");
            out
        }
    };
    let v = validate(json!({"expression": "max(series.fps) / load_time"})).await;
    assert_eq!(v["valid"], true, "{v}");
    assert_eq!(
        (v["kind"].as_str(), v["inferred_unit"].as_str()),
        (Some("scalar"), Some("fps/s"))
    );
    assert_eq!(v["inputs"]["series"], json!(["fps"]));
    let v = validate(json!({"expression": "load_time +"})).await;
    assert_eq!(v["valid"], false);
    assert_eq!(v["diagnostics"][0]["start"], 11);
    let v = validate(json!({"expression": "load_time + fps_typo"})).await;
    assert_eq!(
        (
            v["diagnostics"][0]["start"].as_u64(),
            v["diagnostics"][0]["end"].as_u64()
        ),
        (Some(12), Some(20))
    );
    let v = validate(json!({"expression": "load_time", "unit": "B"})).await;
    assert_eq!(v["valid"], false);
    let v = validate(json!({"expression": "rate(series.fps)", "unit": "fps/s"})).await;
    assert_eq!(
        (
            v["valid"].as_bool(),
            v["kind"].as_str(),
            v["x_unit"].as_str()
        ),
        (Some(true), Some("series"), Some("s"))
    );

    let save = |name: &str, body: Value| {
        let ui = ui.clone();
        let url = app.u(&format!("/derived-metrics/{name}"));
        async move { send(ui.client.put(&url).json(&body)).await }
    };
    let (status, out) = save(
        "peak",
        json!({"expression": "max(series.fps)", "unit": "fps"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    assert_eq!(
        (out["version"].as_i64(), out["created"].as_bool()),
        (Some(1), Some(true))
    );
    let (status, out) = save(
        "peak",
        json!({"expression": "max(series.fps)", "unit": "fps"}),
    )
    .await;
    assert_eq!(
        (status, out["created"].as_bool()),
        (StatusCode::OK, Some(false))
    );

    // Saving is not reachable with a token.
    let response = write
        .raw(
            write
                .client
                .put(app.m("/derived-metrics/peak"))
                .json(&json!({"expression": "1", "unit": ""})),
        )
        .await;
    assert!(!response.status().is_success(), "{}", response.status());

    assert_eq!(
        save(
            "double",
            json!({"expression": "load_time * 2", "unit": "ms"})
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (status, out) = save(
        "plus",
        json!({"expression": "derived.double + 1[s]", "unit": "s", "description": "d"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    assert_eq!(out["uses"][0]["name"], "double");
    assert_eq!(out["uses"][0]["version"], 1);
    let (status, out) = save(
        "double",
        json!({"expression": "load_time * 3", "unit": "ms"}),
    )
    .await;
    assert_eq!(
        (status, out["version"].as_i64()),
        (StatusCode::CREATED, Some(2))
    );

    // plus keeps using double v1 (pinned): 1.5 * 2 + 1 and 1.25 * 2 + 1.
    let (status, out) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": ce, "name": "plus"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["unit"], "s");
    assert!(
        close(&out["samples"][0]["value"], 4.0) && close(&out["samples"][1]["value"], 3.5),
        "{out}"
    );
    // An ad-hoc expression uses the latest version.
    let (_, out) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": ce, "expression": "derived.double", "unit": "s"}),
        )
        .await;
    assert!(close(&out["samples"][0]["value"], 4.5), "{out}");

    // Cycles and self references are rejected.
    let (status, out) = save("double", json!({"expression": "derived.plus", "unit": "s"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{out}");
    assert!(
        out["detail"]
            .as_str()
            .unwrap()
            .contains("cannot use itself"),
        "{out}"
    );
    let (status, _) = save("me", json!({"expression": "derived.me", "unit": ""})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = save("bad", json!({"expression": "load_time", "unit": "B"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, list) = read.get(&app.m("/derived-metrics")).await;
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["double", "peak", "plus"]);
    let (status, one) = read.get(&app.m("/derived-metrics/double")).await;
    assert_eq!(
        (status, one["versions"].as_array().unwrap().len()),
        (StatusCode::OK, 2)
    );
    assert_eq!(
        read.get(&app.m("/derived-metrics/none")).await.0,
        StatusCode::NOT_FOUND
    );

    // No value (FR-12a): a threshold that is never reached.
    let (_, out) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": ce, "expression": "time_to(series.fps, 1000)"}),
        )
        .await;
    assert_eq!(out["samples"][0]["status"], "no_value", "{out}");
    assert_eq!(out["samples"][0]["value"], Value::Null);
    let (_, out) = read
        .post(&app.m("/derived-metrics/evaluate"), &json!({"case_execution_id": ce, "expression": "time_to(series.fps, 50)", "unit": "ms"}))
        .await;
    assert!(close(&out["samples"][0]["value"], 2500.0 / 3.0), "{out}");
    let (_, out) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": ce, "expression": "series.fps * 2", "max_points": 2}),
        )
        .await;
    assert_eq!(out["kind"], "series");
    assert!(out["samples"][0]["series"]["y"].as_array().unwrap().len() <= 2);
    let (_, out) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": ce, "expression": "frames"}),
        )
        .await;
    assert_eq!(out["samples"][0]["status"], "missing");
    let (status, _) = read
        .post(
            &app.m("/derived-metrics/evaluate"),
            &json!({"case_execution_id": Uuid::new_v4(), "expression": "load_time"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Derived metrics in aggregates, with no value counted.
    let (_, out) = ui
        .post(
            &app.u("/analysis/aggregates"),
            &json!({"metric": "derived.plus"}),
        )
        .await;
    assert!(close(&out["groups"][0]["stats"]["mean"], 3.75), "{out}");
    let (_, out) = ui
        .post(
            &app.u("/analysis/aggregates"),
            &json!({"metric": "time_to(series.fps, 1000)"}),
        )
        .await;
    let st = &out["groups"][0]["stats"];
    assert_eq!(
        (st["n"].as_u64(), st["n_no_value"].as_u64()),
        (Some(0), Some(2)),
        "{out}"
    );
    assert_eq!(st["mean"], Value::Null);
}

#[tokio::test]
async fn trend_marks_changes_and_follows_aliases() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let r1 = new_run(&write, &app, "2026-11-01T10:00:00Z", "abc").await;
    let ce1 = add_ce(
        &write,
        &app,
        r1,
        Ce {
            device: "pixel-8-a",
            ..Default::default()
        },
        json!([s(0, 1.0, false), s(1, 3.0, false)]),
    )
    .await;
    let r2 = new_run(&write, &app, "2026-11-02T10:00:00Z", "abc").await;
    add_ce(&write, &app, r2, Ce::default(), json!([s(0, 4.0, false)])).await;
    let r3 = new_run(&write, &app, "2026-11-03T10:00:00Z", "def").await;
    add_ce(
        &write,
        &app,
        r3,
        Ce {
            os: "Android 16",
            ..Default::default()
        },
        json!([s(0, 8.0, false)]),
    )
    .await;
    // Another scenario version: a different case, never mixed in.
    let r4 = new_run(&write, &app, "2026-11-04T10:00:00Z", "def").await;
    let mut body = case_execution_body("pixel-8", 10, json!([s(0, 99.0, false)]));
    body["test_case"]["scenario"]["definition"] = json!({"steps": ["open"]});
    let (status, _) = write
        .put(
            &app.m(&format!("/runs/{r4}/case-executions/{}", Uuid::new_v4())),
            &body,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = ui
        .put(
            &app.u("/aliases"),
            &json!({"dimension": "client_device", "source": "pixel-8-a", "target": "pixel-8"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = ui
        .post(
            &app.u("/annotations"),
            &json!({"run_id": r2, "body": "new driver"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    ui.post(
        &app.u("/annotations"),
        &json!({"occurred_at": "2026-11-02T12:00:00Z", "body": "switched compression"}),
    )
    .await;
    ui.post(
        &app.u("/annotations"),
        &json!({"occurred_at": "2026-12-24T12:00:00Z", "body": "outside"}),
    )
    .await;

    let (_, detail) = ui.get(&app.u(&format!("/case-executions/{ce1}"))).await;
    let test_case_id = detail["test_case"]["id"].as_i64().unwrap();

    let trend = |extra: Value| {
        let ui = ui.clone();
        let url = app.u("/analysis/trend");
        let mut body = json!({"test_case_id": test_case_id, "client_device": "pixel-8", "metric": "load_time"});
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        async move {
            let (status, out) = ui.post(&url, &body).await;
            assert_eq!(status, StatusCode::OK, "{out}");
            out
        }
    };
    let out = trend(json!({})).await;
    let points = out["points"].as_array().unwrap();
    let runs: Vec<&str> = points
        .iter()
        .map(|p| p["run"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        runs,
        vec![r1.to_string(), r2.to_string(), r3.to_string()],
        "{out}"
    );
    assert!(close(&points[0]["stats"]["mean"], 2.0));
    assert_eq!(points[0]["markers"], json!([]));
    assert_eq!(
        points[1]["markers"],
        json!([]),
        "alias merged pixel-8-a into pixel-8"
    );
    assert_eq!(
        points[2]["markers"],
        json!(["device_details_changed", "sut_version_changed"])
    );
    assert_eq!(out["client_device"], "pixel-8");
    let mut bodies: Vec<&str> = out["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["body"].as_str().unwrap())
        .collect();
    bodies.sort();
    assert_eq!(bodies, vec!["new driver", "switched compression"]);

    let out = trend(json!({"reference_run_id": r1, "unit": "ms"})).await;
    assert!(close(&out["points"][2]["relative_to_reference"], 4.0));
    assert!(close(&out["points"][0]["stats"]["mean"], 2000.0));
    let out = trend(json!({"client_device": "pixel-8-a"})).await;
    assert_eq!(out["points"].as_array().unwrap().len(), 3);
    let out = trend(json!({"filter": {"started_after": "2026-11-02T00:00:00Z"}})).await;
    assert_eq!(out["points"].as_array().unwrap().len(), 2);
    let out = trend(json!({"order_by": "sut_version"})).await;
    assert_eq!(out["points"][2]["run"]["id"], r3.to_string());

    let (status, _) = ui
        .post(
            &app.u("/analysis/trend"),
            &json!({"test_case_id": 999999, "client_device": "x", "metric": "load_time"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn ndjson(api: &Api, url: &str, body: &Value) -> (StatusCode, Vec<Value>) {
    let response = api.raw(api.client.post(url).json(body)).await;
    let status = response.status();
    let content_type = response.headers().get("content-type").cloned();
    let text = response.text().await.unwrap();
    if status != StatusCode::OK {
        return (status, vec![serde_json::from_str(&text).unwrap()]);
    }
    assert_eq!(content_type.unwrap(), "application/x-ndjson");
    (
        status,
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect(),
    )
}

fn row_events(events: &[Value]) -> Vec<&Value> {
    events.iter().filter(|e| e["type"] == "row").collect()
}

#[tokio::test]
async fn compare_never_pools_runs_and_marks_differences() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let a = new_run(&write, &app, "2026-11-01T10:00:00Z", "abc").await;
    add_ce(
        &write,
        &app,
        a,
        Ce::default(),
        json!([s(0, 1.0, false), s(1, 3.0, false)]),
    )
    .await;
    add_ce(
        &write,
        &app,
        a,
        Ce {
            pipeline: "new",
            size_mb: 20,
            ..Default::default()
        },
        json!([s(0, 2.0, false), s(1, 2.0, false)]),
    )
    .await;
    let b = new_run(&write, &app, "2026-11-02T10:00:00Z", "def").await;
    add_ce(
        &write,
        &app,
        b,
        Ce::default(),
        json!([s(0, 3.0, false), s(1, 5.0, false)]),
    )
    .await;
    add_ce(
        &write,
        &app,
        b,
        Ce {
            device: "iphone",
            ..Default::default()
        },
        json!([s(0, 10.0, false)]),
    )
    .await;
    let url = app.u("/analysis/compare");

    // Run A vs. run B.
    let (status, events) = ndjson(
        &ui,
        &url,
        &json!({"metric": "load_time", "dimension": "run"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    assert_eq!(events[0]["type"], "start");
    assert_eq!(events[0]["values"], json!([a.to_string(), b.to_string()]));
    assert_eq!(events[0]["reference"], a.to_string());
    assert!(events.iter().any(|e| e["type"] == "progress"));
    assert_eq!(events.last().unwrap()["type"], "done");
    let rows = row_events(&events);
    assert_eq!(rows.len(), 3, "{rows:?}");
    let classic_pixel = rows
        .iter()
        .find(|r| {
            r["case"]["parameters"]["pipeline"] == "classic" && r["client_device"] == "pixel-8"
        })
        .unwrap();
    let cells = classic_pixel["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0]["is_reference"], true);
    assert!(close(&cells[0]["runs"][0]["stats"]["mean"], 2.0));
    assert!(close(&cells[1]["runs"][0]["stats"]["mean"], 4.0));
    let d = &cells[1]["difference"];
    assert!(
        close(&d["absolute"], 2.0) && close(&d["relative"], 1.0) && close(&d["ratio"], 2.0),
        "{d}"
    );
    assert_eq!(cells[1]["sut_version_differs"], true);
    assert_eq!(cells[1]["shared_environment_differs"], false);
    let iphone = rows
        .iter()
        .find(|r| r["client_device"] == "iphone")
        .unwrap();
    assert_eq!(
        iphone["cells"][0]["difference"],
        Value::Null,
        "no reference cell"
    );

    // Devices: a cell holds one aggregate per run, in time order.
    let (_, events) = ndjson(
        &ui,
        &url,
        &json!({"metric": "load_time", "dimension": "client_device", "reference": "pixel-8",
                "filter": {"param": ["pipeline=classic"]}}),
    )
    .await;
    let rows = row_events(&events);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["client_device"], Value::Null);
    let cells = rows[0]["cells"].as_array().unwrap();
    assert_eq!(cells[0]["value"], "iphone");
    let pixel = &cells[1];
    assert_eq!(pixel["is_reference"], true);
    let means: Vec<f64> = pixel["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["stats"]["mean"].as_f64().unwrap())
        .collect();
    assert_eq!(means, vec![2.0, 4.0], "never pooled");
    assert!(
        close(&cells[0]["difference"]["absolute"], 6.0),
        "latest run vs latest run"
    );

    // Parameters, with size taken out of the matching key.
    let body = json!({"metric": "load_time", "dimension": "parameters", "parameters": ["pipeline"],
                      "filter": {"run_id": [a]}, "exclude_from_key": ["size_mb"],
                      "reference": {"pipeline": "classic"}});
    let (_, events) = ndjson(&ui, &url, &body).await;
    let rows = row_events(&events);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["case"]["parameters"], json!({}));
    let cells = rows[0]["cells"].as_array().unwrap();
    assert_eq!(cells[0]["value"], json!({"pipeline": "classic"}));
    assert_eq!(
        cells[1]["runs"][0]["excluded_parameters"],
        json!([{"size_mb": 20}])
    );
    assert!(close(&cells[1]["difference"]["absolute"], 0.0));
    // Without excluding size, the two pipelines do not match.
    let mut body2 = body.clone();
    body2["exclude_from_key"] = json!([]);
    let (_, events) = ndjson(&ui, &url, &body2).await;
    assert_eq!(row_events(&events).len(), 2);

    // No value: the difference is not computable.
    let (_, events) = ndjson(
        &ui,
        &url,
        &json!({"metric": "time_to(series.fps, 1000)", "dimension": "run"}),
    )
    .await;
    let rows = row_events(&events);
    let row = rows
        .iter()
        .find(|r| r["cells"].as_array().unwrap().len() == 2)
        .unwrap();
    assert_eq!(row["cells"][1]["not_computable"], true);
    assert_eq!(row["cells"][1]["difference"], Value::Null);
    assert_eq!(row["cells"][1]["runs"][0]["stats"]["n_no_value"], 2);

    // Request errors come before the stream.
    let (status, _) = ndjson(
        &ui,
        &url,
        &json!({"metric": "load_time", "dimension": "run", "reference": Uuid::new_v4()}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = ndjson(
        &ui,
        &url,
        &json!({"metric": "load_time", "dimension": "parameters"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn series_views_and_downsampling() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let (_, ce) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;

    let (status, out) = ui
        .post(
            &app.u("/analysis/series/overlay"),
            &json!({"metric": "series.fps"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    let series = out["series"].as_array().unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0]["x"].as_array().unwrap().len(), 5);
    assert_eq!(
        (out["x_unit"].as_str(), out["x_dimension"].as_str()),
        (Some("s"), Some("time"))
    );

    let (_, out) = ui
        .post(
            &app.u("/analysis/series/overlay"),
            &json!({"metric": "series.fps", "x_unit": "ms",
              "grid": {"from": 0, "to": 2000, "points": 5}}),
        )
        .await;
    assert_eq!(
        out["grid"],
        json!([0.0, 500.0, 1000.0, 1500.0, 2000.0]),
        "{out}"
    );
    assert_eq!(out["series"][0]["x"], Value::Null);
    assert!(close(&out["series"][0]["y"][3], 56.5));

    let (_, out) = ui
        .post(
            &app.u("/analysis/series/overlay"),
            &json!({"metric": "series.fps", "max_series": 1, "max_points": 2}),
        )
        .await;
    assert_eq!(
        (
            out["series"].as_array().unwrap().len(),
            out["truncated"].as_bool()
        ),
        (1, Some(true))
    );
    assert!(out["series"][0]["y"].as_array().unwrap().len() <= 2);

    let (status, out) = ui
        .post(
            &app.u("/analysis/series/aggregate"),
            &json!({"metric": "series.fps * 2", "grid": {"points": 3}, "percentiles": [50]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    let g = &out["groups"][0];
    assert_eq!(
        (g["n_series"].as_u64(), g["x"].as_array().unwrap().len()),
        (Some(2), 3)
    );
    assert_eq!(g["n"], json!([2, 2, 2]));
    assert!(close(&g["percentiles"][0]["values"][1], 110.0), "{g}");

    let (status, _) = ui
        .post(
            &app.u("/analysis/series/overlay"),
            &json!({"metric": "load_time"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = ui
        .post(
            &app.u("/analysis/series/overlay"),
            &json!({"metric": "series.fps", "x_unit": "B"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Point retrieval with downsampling.
    let (_, detail) = ui.get(&app.u(&format!("/case-executions/{ce}"))).await;
    let series_id = detail["samples"][0]["series"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, points) = ui
        .get(&app.u(&format!("/series/{series_id}/points?max_points=2")))
        .await;
    assert_eq!(points["y"].as_array().unwrap().len(), 2, "{points}");
    assert!(
        points["y"].as_array().unwrap().contains(&json!(60.0)),
        "keeps the maximum"
    );
    let (status, _) = ui
        .get(&app.u(&format!("/series/{series_id}/points?max_points=1")))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn export_round_trips() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let (run, _) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;

    let export = |body: Value| {
        let ui = ui.clone();
        let url = app.u("/analysis/export");
        async move {
            let response = ui.raw(ui.client.post(&url).json(&body)).await;
            assert_eq!(response.status(), StatusCode::OK);
            let headers = response.headers().clone();
            assert!(
                headers["content-disposition"]
                    .to_str()
                    .unwrap()
                    .starts_with("attachment")
            );
            (
                headers["content-type"].to_str().unwrap().to_owned(),
                response.bytes().await.unwrap(),
            )
        }
    };

    let (ct, bytes) = export(json!({"metric": "load_time", "unit": "ms", "format": "csv"})).await;
    assert!(ct.starts_with("text/csv"));
    let mut reader = csv::Reader::from_reader(bytes.as_ref());
    let headers = reader.headers().unwrap().clone();
    let value_col = headers.iter().position(|h| h == "value").unwrap();
    let run_col = headers.iter().position(|h| h == "run_id").unwrap();
    let records: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
    assert_eq!(records.len(), 2);
    let mut values: Vec<f64> = records
        .iter()
        .map(|r| r[value_col].parse().unwrap())
        .collect();
    values.sort_by(f64::total_cmp);
    assert_eq!(values, vec![1250.0, 1500.0]);
    assert_eq!(&records[0][run_col], run.to_string());

    let (ct, bytes) = export(json!({"metric": "load_time", "format": "json"})).await;
    assert_eq!(ct, "application/x-ndjson");
    let lines: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["client_device"], "pixel-8");
    assert_eq!(lines[0]["unit"], "s");
    assert_eq!(
        serde_json::from_str::<Value>(lines[0]["parameters"].as_str().unwrap()).unwrap()["size_mb"],
        10
    );

    let (ct, bytes) =
        export(json!({"metric": "time_to(series.fps, 1000)", "format": "parquet"})).await;
    assert_eq!(ct, "application/vnd.apache.parquet");
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(bytes)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.map(|b| b.unwrap()).collect();
    assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 2);
    let has_value = batches[0].column_by_name("has_value").unwrap();
    assert_eq!(has_value.null_count(), 0);
    assert_eq!(
        batches[0].column_by_name("value").unwrap().null_count(),
        2,
        "no value is null"
    );

    let (_, bytes) = export(
        json!({"metric": "load_time", "format": "csv", "level": "aggregates", "percentiles": [50]}),
    )
    .await;
    let mut reader = csv::Reader::from_reader(bytes.as_ref());
    let headers = reader.headers().unwrap().clone();
    let records: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
    assert_eq!(records.len(), 1);
    let at = |name: &str| headers.iter().position(|h| h == name).unwrap();
    assert_eq!(&records[0][at("mean")], "1.375");
    assert_eq!(&records[0][at("n")], "2");
    assert_eq!(&records[0][at("p50")], "1.375");

    // An empty selection still has a header.
    let (_, bytes) =
        export(json!({"metric": "load_time", "format": "csv", "filter": {"suite": "none"}})).await;
    assert!(std::str::from_utf8(&bytes).unwrap().starts_with("run_id,"));

    let (status, _) = ui
        .post(
            &app.u("/analysis/export"),
            &json!({"metric": "series.fps", "format": "csv"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
