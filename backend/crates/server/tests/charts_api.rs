//! The chart endpoints (FR-20, FR-22, FR-23, ADR 0022): `POST /charts/run`
//! and `POST /charts/validate` on both ports, and the snapshot a run reads.

mod common;

use bytes::Bytes;
use reqwest::StatusCode;
use serde_json::{Value, json};
use track_record_server::db::TokenScope;
use track_record_server::service::charts::{self, source};
use uuid::Uuid;

use common::*;

/// The trend of ADR 0022: the mean of `load_time` per run, case and device.
fn trend() -> Value {
    json!({
        "format": 1,
        "variables": [{"name": "branch", "type": "text", "default": "main"}],
        "nodes": [
            {"id": "runs", "type": "source", "metric": "load_time",
             "filter": {"suite": "nightly", "sut_branch": {"var": "branch"}}},
            {"id": "lines", "type": "group", "input": "runs", "by": ["case", "device.name"]},
            {"id": "per_run", "type": "group", "input": "lines", "by": ["run.id"]},
            {"id": "samples", "type": "concatenate", "input": "per_run"},
            {"id": "ordered", "type": "sort", "input": "samples", "by": [{"key": "run.started_at"}]},
            {"id": "stats", "type": "reduce", "input": "ordered", "functions": ["mean"]},
            {"id": "chart", "type": "line", "input": "stats", "x": "run.started_at", "y": "mean"}
        ]
    })
}

/// The `fps` series of every sample, one line each.
fn fps_lines() -> Value {
    json!({
        "format": 1,
        "nodes": [
            {"id": "fps", "type": "source", "metric": "series.fps"},
            {"id": "chart", "type": "line", "input": "fps", "x": "x", "y": "y"}
        ]
    })
}

/// Two runs of one case on one device, a day apart; each has samples of
/// 1.5 s and 1.25 s with an `fps` series of 5 points.
async fn seed(app: &TestApp) -> (Uuid, Uuid) {
    let write = app.as_scope(TokenScope::Write).await;
    let (r1, _) = seed_run(&write, app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let (r2, _) = seed_run(&write, app, "pixel-8", "2026-11-02T10:00:00Z").await;
    (r1, r2)
}

fn types(events: &[Value]) -> Vec<&str> {
    events.iter().map(|e| e["type"].as_str().unwrap()).collect()
}

fn result(events: &[Value]) -> &Value {
    events
        .iter()
        .find(|e| e["type"] == "result")
        .unwrap_or_else(|| panic!("no result: {events:#?}"))
}

/// The values of column `name` in a frame of the result.
fn column(frame: &Value, name: &str) -> Vec<Value> {
    let i = frame["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no column {name}: {frame}"));
    frame["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["values"][i].clone())
        .collect()
}

#[tokio::test]
async fn a_chart_runs_on_both_ports_with_every_scope() {
    let app = app!();
    let (r1, r2) = seed(&app).await;
    let body = json!({"spec": trend()});

    let mut runs = vec![ndjson(&app.ui_client(), &app.u("/charts/run"), &body).await];
    for scope in [TokenScope::Read, TokenScope::Annotate, TokenScope::Write] {
        let api = app.as_scope(scope).await;
        runs.push(ndjson(&api, &app.m("/charts/run"), &body).await);
    }
    for (status, events) in &runs {
        assert_eq!(*status, StatusCode::OK, "{events:?}");
        let t = types(events);
        assert_eq!(t[0], "start");
        assert_eq!(t[t.len() - 2..], ["result", "done"]);
        assert!(t.contains(&"progress"), "{t:?}");
        assert_eq!(
            events[0],
            json!({"type": "start", "node": "chart", "partial": false,
                   "sources": [{"id": "runs", "samples": 4}], "total_samples": 4})
        );
        let progress = events.iter().rfind(|e| e["type"] == "progress").unwrap();
        assert_eq!(
            (&progress["done"], &progress["total"]),
            (&json!(4), &json!(4))
        );

        let result = result(events);
        assert_eq!(
            (&result["node"], &result["partial"]),
            (&json!("chart"), &json!(false))
        );
        assert_eq!(result["excluded"]["runs"]["missing"], 0);
        let frames = result["tree"]["frames"].as_array().unwrap();
        assert_eq!(frames.len(), 1, "one line: {result}");
        let line = &frames[0];
        assert_eq!(line["attributes"]["device.name"]["value"], "pixel-8");
        assert_eq!(column(line, "mean"), [json!(1.375), json!(1.375)]);
        assert_eq!(column(line, "n"), [json!(2.0), json!(2.0)]);
        let unit = |name: &str| {
            line["columns"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap()["unit"]
                .clone()
        };
        assert_eq!(unit("mean"), "s");
        // A point holds the attributes in which it differs from its line.
        let points = line["points"].as_array().unwrap();
        assert_eq!(points[0]["attributes"]["run.id"]["value"], r1.to_string());
        assert_eq!(points[1]["attributes"]["run.id"]["value"], r2.to_string());
        assert!(
            points[0]["attributes"]["run.started_at"]["value"]
                .as_str()
                .unwrap()
                .starts_with("2026-11-01T10:00:00")
        );
        assert!(line["attributes"].get("run.id").is_none());
    }

    // The machine port needs a token.
    let anonymous = app.ui_client();
    let (status, _) = anonymous.post(&app.m("/charts/run"), &body).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn request_errors_come_before_the_stream() {
    let app = app!();
    seed(&app).await;
    let ui = app.ui_client();
    let url = app.u("/charts/run");
    let problem = |events: &[Value]| {
        (
            events[0]["code"].as_str().unwrap().to_owned(),
            events[0]["detail"].as_str().unwrap().to_owned(),
        )
    };

    // Not well-formed: an unknown node type.
    let mut spec = trend();
    spec["nodes"][1]["type"] = json!("explode");
    let (status, events) = ndjson(&ui, &url, &json!({"spec": spec})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (code, detail) = problem(&events);
    assert_eq!(code, "invalid_chart");
    assert!(detail.contains("explode"), "{detail}");

    // Not valid: a metric that does not exist.
    let mut spec = trend();
    spec["nodes"][0]["metric"] = json!("no_such_metric");
    let (status, events) = ndjson(&ui, &url, &json!({"spec": spec})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (code, detail) = problem(&events);
    assert_eq!(code, "invalid_chart");
    assert!(detail.contains("node \"runs\""), "{detail}");

    // A variable without a value and without a default.
    let mut spec = trend();
    spec["variables"][0] = json!({"name": "branch", "type": "text"});
    let (status, events) = ndjson(&ui, &url, &json!({"spec": spec.clone()})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(problem(&events).1.contains("branch"), "{events:?}");
    // With a value it runs.
    let body = json!({"spec": spec, "variables": {"branch": "main"}});
    let (status, events) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    // A value of the wrong type.
    let body = json!({"spec": trend(), "variables": {"branch": 3}});
    let (status, _) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // A node that does not exist.
    let body = json!({"spec": trend(), "until": "nowhere"});
    let (status, events) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(problem(&events).1.contains("nowhere"), "{events:?}");

    // Options that do not go together, or out of range.
    for body in [
        json!({"spec": trend(), "format": "csv", "preview": true}),
        json!({"spec": trend(), "format": "csv", "until": "stats"}),
        json!({"spec": trend(), "max_points": 1}),
    ] {
        let (status, events) = ndjson(&ui, &url, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {events:?}");
    }
    // Paging is not part of a source's filter.
    let mut spec = trend();
    spec["nodes"][0]["filter"] = json!({"limit": 1});
    let (status, events) = ndjson(&ui, &url, &json!({"spec": spec})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem(&events).0, "invalid_chart");
    // Unknown fields are rejected.
    let (status, _) = ndjson(&ui, &url, &json!({"spec": trend(), "speed": "fast"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn an_error_during_evaluation_ends_the_stream() {
    let app = app!();
    seed(&app).await;
    // Matching by case alone finds two samples of the reference per case:
    // the data does not fit the match, which only the run can tell.
    let spec = json!({
        "format": 1,
        "nodes": [
            {"id": "runs", "type": "source", "metric": "load_time"},
            {"id": "ref", "type": "source", "metric": "load_time"},
            {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
             "only": ["case"]},
            {"id": "ratio", "type": "combine", "input": "pairs",
             "expressions": [{"name": "ratio", "expression": "a.y / b.y"}]},
            {"id": "chart", "type": "line", "input": "ratio", "x": "run.started_at", "y": "ratio"}
        ]
    });
    let (status, events) = ndjson(
        &app.ui_client(),
        &app.u("/charts/run"),
        &json!({"spec": spec}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    assert_eq!(types(&events).first(), Some(&"start"));
    let last = events.last().unwrap();
    assert_eq!(last["type"], "error", "{events:#?}");
    assert_eq!(last["code"], "chart_failed");
    assert_eq!(last["node"], "pairs");
    assert!(!types(&events).contains(&"result"));
}

#[tokio::test]
async fn a_preview_reads_the_latest_run_and_stops_at_a_node() {
    let app = app!();
    let (_, r2) = seed(&app).await;
    let ui = app.ui_client();
    let url = app.u("/charts/run");

    let (status, events) = ndjson(&ui, &url, &json!({"spec": trend(), "preview": true})).await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    assert_eq!(events[0]["partial"], true);
    assert_eq!(events[0]["total_samples"], 2);
    let preview = result(&events);
    assert_eq!(preview["partial"], true);
    let line = &preview["tree"]["frames"][0];
    assert_eq!(column(line, "mean"), [json!(1.375)]);
    assert_eq!(line["attributes"]["run.id"]["value"], r2.to_string());

    // Up to a node: its output, with its groups.
    let body = json!({"spec": trend(), "until": "samples"});
    let (status, events) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    assert_eq!(
        (&events[0]["node"], &events[0]["partial"]),
        (&json!("samples"), &json!(true))
    );
    let groups = result(&events)["tree"]["groups"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(groups.len(), 1, "one line");
    let key: Vec<&Value> = groups[0]["key"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| &k["name"])
        .collect();
    assert!(key.contains(&&json!("device.name")), "{key:?}");
    let frames = groups[0]["tree"]["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2, "a frame per run");
    assert_eq!(column(&frames[0], "y"), [json!(1.5), json!(1.25)]);

    // The problems of later nodes do not hold up a preview of this one.
    let mut spec = trend();
    spec["nodes"][6]["y"] = json!("no_such_column");
    let body = json!({"spec": spec.clone(), "until": "stats"});
    let (status, events) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    let (status, _) = ndjson(&ui, &url, &json!({"spec": spec})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn lines_are_downsampled_for_display() {
    let app = app!();
    seed(&app).await;
    let ui = app.ui_client();
    let url = app.u("/charts/run");
    let points = |events: &[Value]| -> Vec<usize> {
        result(events)["tree"]["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["points"].as_array().unwrap().len())
            .collect()
    };

    let (_, events) = ndjson(&ui, &url, &json!({"spec": fps_lines()})).await;
    assert_eq!(points(&events), [5, 5, 5, 5]);
    let (_, events) = ndjson(&ui, &url, &json!({"spec": fps_lines(), "max_points": 2})).await;
    assert_eq!(points(&events), [2, 2, 2, 2]);
    // The minimum and the maximum stay, with their x.
    let frame = &result(&events)["tree"]["frames"][0];
    assert_eq!(column(frame, "y"), [json!(10.0), json!(60.0)]);
    assert_eq!(column(frame, "x"), [json!(0.0), json!(1.0)]);
}

async fn export(app: &TestApp, format: &str) -> (String, String, Bytes) {
    let ui = app.ui_client();
    let body = json!({"spec": trend(), "format": format});
    let response = ui
        .raw(ui.client.post(app.u("/charts/run")).json(&body))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    };
    let (content_type, disposition) = (header("content-type"), header("content-disposition"));
    (content_type, disposition, response.bytes().await.unwrap())
}

#[tokio::test]
async fn the_data_of_a_chart_exports_as_a_file() {
    let app = app!();
    let (r1, r2) = seed(&app).await;

    let (content_type, disposition, bytes) = export(&app, "csv").await;
    assert_eq!(content_type, "text/csv; charset=utf-8");
    assert_eq!(
        disposition,
        "attachment; filename=\"track-record-chart.csv\""
    );
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let header: Vec<String> = reader
        .headers()
        .unwrap()
        .iter()
        .map(str::to_owned)
        .collect();
    for name in [
        "run.id",
        "device.name",
        "param.size_mb",
        "unit(param.size_mb)",
        "mean",
        "unit(mean)",
        "n",
    ] {
        assert!(header.contains(&name.to_owned()), "{name} in {header:?}");
    }
    let rows: Vec<csv::StringRecord> = reader.records().map(Result::unwrap).collect();
    assert_eq!(rows.len(), 2, "a row per point:\n{text}");
    let at = |row: &csv::StringRecord, name: &str| {
        row[header.iter().position(|h| h == name).unwrap()].to_owned()
    };
    assert_eq!(at(&rows[0], "run.id"), r1.to_string());
    assert_eq!(at(&rows[1], "run.id"), r2.to_string());
    assert_eq!(at(&rows[0], "mean"), "1.375");
    assert_eq!(at(&rows[0], "unit(mean)"), "s");
    assert_eq!(at(&rows[0], "param.size_mb"), "10");
    assert_eq!(at(&rows[0], "unit(param.size_mb)"), "MB");

    let (content_type, disposition, bytes) = export(&app, "json").await;
    assert_eq!(content_type, "application/x-ndjson");
    assert!(disposition.ends_with(".ndjson\""), "{disposition}");
    let rows: Vec<Value> = String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1]["run.id"], r2.to_string());
    assert_eq!(rows[1]["mean"], 1.375);
    assert_eq!(rows[1]["param.size_mb"], 10);

    let (content_type, disposition, bytes) = export(&app, "parquet").await;
    assert_eq!(content_type, "application/vnd.apache.parquet");
    assert!(disposition.ends_with(".parquet\""), "{disposition}");
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(bytes)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<_> = reader.map(Result::unwrap).collect();
    assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 2);
    let schema = batches[0].schema();
    assert!(schema.field_with_name("mean").is_ok());
    assert!(schema.field_with_name("run.started_at").is_ok());
}

#[tokio::test]
async fn too_many_samples_are_rejected_before_the_stream() {
    let app = app!(|s: &mut track_record_server::AppState| s.chart_limits.max_samples = 3);
    seed(&app).await;
    let (status, events) = ndjson(
        &app.ui_client(),
        &app.u("/charts/run"),
        &json!({"spec": trend()}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let detail = events[0]["detail"].as_str().unwrap();
    assert!(
        detail.contains("more than 3 samples; narrow it"),
        "{detail}"
    );
    // A preview reads fewer.
    let body = json!({"spec": trend(), "preview": true});
    let (status, _) = ndjson(&app.ui_client(), &app.u("/charts/run"), &body).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_run_over_the_budget_of_points_fails_and_says_why() {
    // Four series of five points: more than the chart may hold.
    let app = app!(|s: &mut track_record_server::AppState| s.chart_limits.max_points = 12);
    seed(&app).await;
    let (status, events) = ndjson(
        &app.ui_client(),
        &app.u("/charts/run"),
        &json!({"spec": fps_lines()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    let last = events.last().unwrap();
    assert_eq!(last["type"], "error", "{events:#?}");
    let detail = last["detail"].as_str().unwrap();
    assert!(detail.contains("narrow the filter"), "{detail}");
    assert!(
        detail.contains("max(series.fps)"),
        "says how to stay below: {detail}"
    );
    // Reduced in the metric expression, it fits.
    let mut spec = fps_lines();
    spec["nodes"][0]["metric"] = json!("max(series.fps)");
    spec["nodes"][1]["x"] = json!("run.started_at");
    let (status, events) = ndjson(
        &app.ui_client(),
        &app.u("/charts/run"),
        &json!({"spec": spec}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events.last().unwrap()["type"], "done", "{events:#?}");
}

#[tokio::test]
async fn validation_reports_problems_and_the_output_of_each_node() {
    let app = app!();
    seed(&app).await;
    let read = app.as_scope(TokenScope::Read).await;
    let ui = app.ui_client();

    for (api, url) in [
        (&ui, app.u("/charts/validate")),
        (&read, app.m("/charts/validate")),
    ] {
        let (status, body) = api.post(&url, &json!({"spec": trend()})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            (&body["well_formed"], &body["valid"]),
            (&json!(true), &json!(true))
        );
        assert_eq!(body["problems"], json!([]));
        let stats = &body["nodes"]["stats"];
        assert!(
            stats["columns"]
                .as_array()
                .unwrap()
                .contains(&json!({"name": "mean", "unit": "s"})),
            "{stats}"
        );
        assert_eq!(stats["levels"], json!([]), "a frame per line");
        assert_eq!(body["nodes"]["samples"]["levels"][0]["kind"], "group");
        assert!(
            body["nodes"]["runs"]["attributes"]
                .as_array()
                .unwrap()
                .contains(&json!("run.id"))
        );
    }

    // A problem in an expression, with its position.
    let mut spec = trend();
    spec["nodes"][0]["metric"] = json!("load_time + nope");
    let (status, body) = ui
        .post(&app.u("/charts/validate"), &json!({"spec": spec}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (&body["well_formed"], &body["valid"]),
        (&json!(true), &json!(false))
    );
    let problem = &body["problems"][0];
    assert_eq!(
        (&problem["node"], &problem["setting"]),
        (&json!("runs"), &json!("metric"))
    );
    assert_eq!(
        (&problem["start"], &problem["end"]),
        (&json!(12), &json!(16))
    );

    // Not well-formed: no visualization.
    let spec = json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"}
    ]});
    let (status, body) = ui
        .post(&app.u("/charts/validate"), &json!({"spec": spec}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["well_formed"], false);
    assert!(!body["problems"].as_array().unwrap().is_empty());
    assert_eq!(body["nodes"], json!({}));

    // A derived metric counts with the units of what it reads.
    let (status, body) = ui
        .put(
            &app.u("/derived-metrics/load_ms"),
            &json!({"expression": "load_time", "unit": "ms"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let mut spec = trend();
    spec["nodes"][0]["metric"] = json!("derived.load_ms");
    let (_, body) = ui
        .post(&app.u("/charts/validate"), &json!({"spec": spec}))
        .await;
    assert_eq!(body["valid"], true, "{body}");
    assert!(
        body["nodes"]["stats"]["columns"]
            .as_array()
            .unwrap()
            .contains(&json!({"name": "mean", "unit": "ms"})),
        "{body}"
    );
}

#[tokio::test]
async fn a_run_reads_its_sources_in_one_snapshot() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let (run, ce) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    // A third sample, without its `fps` series (yet).
    let sample_id = Uuid::new_v4();
    let (status, body) = write
        .put(
            &app.m(&format!(
                "/runs/{run}/case-executions/{ce}/samples/{sample_id}"
            )),
            &json!({"repetition": 2, "measurements": {"load_time": 2.0}, "metrics": metrics()}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let node = serde_json::from_value(json!({"metric": "max(series.fps)"})).unwrap();
    let limits = source::Limits::default();

    let mut snapshot = charts::snapshot(&app.pool).await.unwrap();
    let selected = source::select(&mut snapshot, "s", &node, &limits, false)
        .await
        .unwrap();
    assert_eq!(selected.samples(), 3);

    // Meanwhile the series arrives, and the run is invalidated.
    let series = json!({
        "metric": "fps", "metrics": metrics(), "x_dimension": "time", "x_unit": "s",
        "x": [0.0, 1.0], "y": [1.0, 2.0]
    });
    let url = format!(
        "/runs/{run}/case-executions/{ce}/samples/{sample_id}/series/{}",
        Uuid::new_v4()
    );
    let (status, body) = write.put(&app.m(&url), &series).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = app
        .ui_client()
        .post(
            &app.u(&format!("/runs/{run}/invalidate")),
            &json!({"reason": "test"}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    // The load still reads the data the selection saw: the third sample
    // has no series.
    let mut budget = source::Budget::new(limits.max_points);
    let loaded = selected
        .load(&mut snapshot, &limits, &mut budget, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(loaded.excluded.missing, 1, "{:?}", loaded.excluded);
    let track_record_core::chart::Tree::Frames(frames) = &loaded.tree else {
        panic!("a flat list")
    };
    assert_eq!(frames.len(), 2);
    assert!(
        frames
            .iter()
            .all(|f| f.attrs["run.valid"].value == track_record_core::chart::AttrValue::Bool(true))
    );
    drop(snapshot);

    // A new run sees both: the series, and the run as invalid (it is left
    // out unless the filter asks for invalid runs).
    let node = serde_json::from_value(json!({
        "metric": "max(series.fps)", "filter": {"validity": "all"}
    }))
    .unwrap();
    let mut conn = app.pool.acquire().await.unwrap();
    let loaded = source::load(&mut conn, "s", &node, &limits, &mut budget, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(loaded.excluded.missing, 0);
    let track_record_core::chart::Tree::Frames(frames) = &loaded.tree else {
        panic!("a flat list")
    };
    assert_eq!(frames.len(), 3);
    assert_eq!(
        frames[0].attrs["run.valid"].value,
        track_record_core::chart::AttrValue::Bool(false)
    );
}

#[tokio::test]
async fn a_preview_is_not_held_up_by_values_of_later_nodes_but_a_run_is() {
    let app = app!();
    seed(&app).await;
    let ui = app.ui_client();
    let url = app.u("/charts/run");
    let spec = |extra: Value| {
        let mut nodes = vec![
            json!({"id": "runs", "type": "source", "metric": "load_time"}),
            json!({"id": "chart", "type": "table", "input": "runs", "columns": ["y"]}),
        ];
        nodes.push(extra);
        json!({"format": 1, "nodes": nodes})
    };
    // A group without keys is a problem of its values, found when binding.
    let bad_values = spec(json!({"id": "loose", "type": "group", "input": "runs", "by": []}));
    let body = json!({"spec": bad_values.clone(), "until": "runs"});
    let (status, events) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    assert_eq!(events.last().unwrap()["type"], "done");
    let (status, events) = ndjson(&ui, &url, &json!({"spec": bad_values})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{events:?}");

    // A node the visualization does not read still has to be valid for a
    // whole run, as validation says.
    let unused = spec(json!({"id": "loose", "type": "group", "input": "runs", "by": ["nothing"]}));
    let (_, report) = ui
        .post(&app.u("/charts/validate"), &json!({"spec": unused.clone()}))
        .await;
    assert_eq!(report["valid"], false, "{report}");
    let (status, events) = ndjson(&ui, &url, &json!({"spec": unused.clone()})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{events:?}");
    assert!(events[0]["detail"].as_str().unwrap().contains("loose"));
    let body = json!({"spec": unused, "until": "chart"});
    let (status, _) = ndjson(&ui, &url, &body).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn validation_checks_parameter_conditions() {
    let app = app!();
    seed(&app).await;
    let mut spec = trend();
    spec["nodes"][0]["filter"] = json!({"param": ["codec"]});
    let (_, body) = app
        .ui_client()
        .post(&app.u("/charts/validate"), &json!({"spec": spec}))
        .await;
    assert_eq!(body["valid"], false, "{body}");
    let problem = &body["problems"][0];
    assert_eq!(problem["node"], "runs");
    assert!(
        problem["setting"]
            .as_str()
            .unwrap()
            .starts_with("filter.param"),
        "{problem}"
    );
}

#[tokio::test]
async fn an_export_keeps_parameter_types_digits_and_names_apart() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let run = Uuid::new_v4();
    let (status, body) = write
        .put(
            &app.m(&format!("/runs/{run}")),
            &run_body("2026-11-01T10:00:00Z"),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // Two cases: `mode` is a number in one and a text in the other, `big`
    // does not fit 64-bit floats, and `size.unit` is a parameter of its own.
    for (mode, big) in [
        (json!(10), json!(9_007_199_254_740_993_u64)),
        (json!("10"), json!(1)),
    ] {
        let mut ce = case_execution_body("pixel-8", 10, json!([sample(0, 1.5)]));
        ce["test_case"]["parameters"] = json!({
            "mode": mode, "big": big, "size": 10, "size.unit": "label"
        });
        ce["test_case"]["parameter_units"] = json!({"size": "MB"});
        let url = format!("/runs/{run}/case-executions/{}", Uuid::new_v4());
        let (status, body) = write.put(&app.m(&url), &ce).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let spec = json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "chart", "type": "table", "input": "runs", "columns": ["y"]}
    ]});
    let ui = app.ui_client();
    let export = |format: &str| {
        let body = json!({"spec": spec.clone(), "format": format});
        let request = ui.client.post(app.u("/charts/run")).json(&body);
        async { ui.raw(request).await.text().await.unwrap() }
    };

    let rows: Vec<Value> = export("json")
        .await
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    let modes: Vec<&Value> = rows.iter().map(|r| &r["param.mode"]).collect();
    assert!(
        modes.contains(&&json!(10)) && modes.contains(&&json!("10")),
        "{modes:?}"
    );
    let bigs: Vec<&Value> = rows.iter().map(|r| &r["param.big"]).collect();
    assert!(
        bigs.contains(&&json!(9_007_199_254_740_993_u64)),
        "{bigs:?}"
    );
    assert_eq!(rows[0]["param.size.unit"], "label");
    assert_eq!(rows[0]["unit(param.size)"], "MB");

    let csv = export("csv").await;
    let header = csv.lines().next().unwrap();
    assert!(header.contains("param.size.unit") && header.contains("unit(param.size)"));
    // Mixed types are JSON text: 10 and "10" stay apart.
    assert!(csv.contains("9007199254740993"), "{csv}");
    assert!(csv.contains(r#""""10""""#), "{csv}");
}

#[tokio::test]
async fn reads_work_with_one_database_connection() {
    use std::time::Duration;
    use track_record_server::service::analysis::{compare, export};
    use track_record_server::service::charts::{model, run, validate};
    use track_record_server::service::derived;

    let app = app!();
    let (_, ce) = {
        let write = app.as_scope(TokenScope::Write).await;
        seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with((*app.pool.connect_options()).clone())
        .await
        .unwrap();

    let req = serde_json::from_value(json!({"case_execution_id": ce, "expression": "load_time"}))
        .unwrap();
    let out = derived::evaluate(&pool, &req).await.unwrap();
    assert_eq!(out.samples.len(), 2);

    let req = serde_json::from_value(json!({
        "metric": "load_time", "dimension": "client_device"
    }))
    .unwrap();
    let mut rx = compare::compare(pool.clone(), req).await.unwrap();
    let mut last = None;
    while let Some(e) = rx.recv().await {
        last = Some(serde_json::to_value(e).unwrap());
    }
    assert_eq!(last.unwrap()["type"], "done");

    let req = serde_json::from_value(json!({"metric": "load_time", "format": "csv"})).unwrap();
    let mut rx = export::export(pool.clone(), req).await.unwrap();
    let mut bytes = 0;
    while let Some(chunk) = rx.recv().await {
        bytes += chunk.unwrap().len();
    }
    assert!(bytes > 0);

    let req = serde_json::from_value(json!({"spec": trend()})).unwrap();
    let report = validate::validate(&pool, &req).await.unwrap();
    assert!(report.valid);
    let req: model::RunChart = serde_json::from_value(json!({"spec": trend()})).unwrap();
    let run::RunOutput::Events(mut rx) = run::run(&pool, source::Limits::default(), req)
        .await
        .unwrap()
    else {
        panic!("events")
    };
    let mut last = None;
    while let Some(e) = rx.recv().await {
        last = Some(serde_json::to_value(e).unwrap());
    }
    assert_eq!(last.unwrap()["type"], "done");
}

#[tokio::test]
async fn validation_and_a_run_agree_on_how_deep_derived_metrics_nest() {
    let app = app!();
    seed(&app).await;
    let ui = app.ui_client();
    // d1 reads load_time, each next one the one before.
    for k in 1..=33 {
        let expression = if k == 1 {
            "load_time".to_owned()
        } else {
            format!("derived.d{}", k - 1)
        };
        let (status, body) = ui
            .put(
                &app.u(&format!("/derived-metrics/d{k}")),
                &json!({"expression": expression, "unit": "s"}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "d{k}: {body}");
    }
    for (metric, ok) in [("derived.d32", true), ("derived.d33", false)] {
        let mut spec = trend();
        spec["nodes"][0]["metric"] = json!(metric);
        let (_, report) = ui
            .post(&app.u("/charts/validate"), &json!({"spec": spec.clone()}))
            .await;
        assert_eq!(report["valid"], ok, "{metric}: {report}");
        let (status, events) = ndjson(&ui, &app.u("/charts/run"), &json!({"spec": spec})).await;
        let expected = if ok {
            StatusCode::OK
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };
        assert_eq!(status, expected, "{metric}: {events:?}");
    }
}

#[tokio::test]
async fn the_sources_of_a_run_share_one_cutoff_for_relative_filters() {
    use track_record_server::service::runs::RunFilter;

    let app = app!();
    let filter: RunFilter = serde_json::from_value(json!({"started_within_days": 1})).unwrap();
    let mut snapshot = charts::snapshot(&app.pool).await.unwrap();
    let first = filter.resolve(&mut snapshot).await.unwrap().started_after;
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let second = filter.resolve(&mut snapshot).await.unwrap().started_after;
    assert!(first.is_some());
    assert_eq!(first, second, "one cutoff within a snapshot");
}

#[tokio::test]
async fn validation_names_the_columns_a_run_gives() {
    let app = app!();
    seed(&app).await;
    let ui = app.ui_client();
    // Two series matched sample by sample, combined without expressions;
    // `b` selects nothing in the second spec, so its column is missing.
    let spec = |b_filter: Value| {
        json!({"format": 1, "nodes": [
            {"id": "a", "type": "source", "metric": "series.fps"},
            {"id": "b", "type": "source", "metric": "series.fps", "filter": b_filter},
            {"id": "pairs", "type": "match", "inputs": {"a": "a", "b": "b"},
             "unmatched": "keep"},
            {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
            {"id": "chart", "type": "line", "input": "both", "x": "x", "y": "b"}
        ]})
    };
    let names = |columns: &Value| -> Vec<String> {
        columns
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let (_, report) = ui
        .post(
            &app.u("/charts/validate"),
            &json!({"spec": spec(json!({}))}),
        )
        .await;
    assert_eq!(report["valid"], true, "{report}");
    assert_eq!(names(&report["nodes"]["both"]["columns"]), ["x", "a", "b"]);
    let (_, events) = ndjson(
        &ui,
        &app.u("/charts/run"),
        &json!({"spec": spec(json!({}))}),
    )
    .await;
    let frame = &result(&events)["tree"]["frames"][0];
    assert_eq!(names(&frame["columns"]), ["x", "a", "b"], "{events:?}");

    // Without its partner, a line of `b` has no values: the display limit
    // still holds.
    let body = json!({"spec": spec(json!({"sut_branch": "nowhere"})), "max_points": 2});
    let (status, events) = ndjson(&ui, &app.u("/charts/run"), &body).await;
    assert_eq!(status, StatusCode::OK, "{events:?}");
    for frame in result(&events)["tree"]["frames"].as_array().unwrap() {
        assert!(frame["points"].as_array().unwrap().len() <= 2, "{frame}");
    }
}

#[tokio::test]
async fn validation_reads_every_name_the_language_takes() {
    let app = app!();
    seed(&app).await;
    let ui = app.ui_client();
    // A derived metric whose reference is exactly as long as the language
    // allows.
    let name = "a".repeat(4096 - "derived.".len());
    let (status, body) = ui
        .put(
            &app.u(&format!("/derived-metrics/{name}")),
            &json!({"expression": "load_time", "unit": "s"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let mut spec = trend();
    spec["nodes"][0]["metric"] = json!(format!("derived.{name}"));
    let (_, report) = ui
        .post(&app.u("/charts/validate"), &json!({"spec": spec.clone()}))
        .await;
    assert_eq!(report["valid"], true, "{report}");
    let (status, _) = ndjson(&ui, &app.u("/charts/run"), &json!({"spec": spec})).await;
    assert_eq!(status, StatusCode::OK);

    // A name no metric can have is unknown, not an error of the server.
    let mut spec = trend();
    spec["nodes"][0]["metric"] = json!("`a\u{0}b`");
    let (status, report) = ui
        .post(&app.u("/charts/validate"), &json!({"spec": spec.clone()}))
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["valid"], false);
    assert_eq!(report["problems"][0]["setting"], "metric");
    let (status, _) = ndjson(&ui, &app.u("/charts/run"), &json!({"spec": spec})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
