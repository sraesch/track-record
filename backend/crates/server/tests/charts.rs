//! Charts (FR-22, ADR 0022): the source of a graph, and graphs run on what
//! sources load.

mod common;

use std::collections::BTreeMap;

use reqwest::StatusCode;
use serde_json::{Value, json};
use track_record_core::chart::spec::{NodeKind, SourceNode};
use track_record_core::chart::{AttrValue, Frame, Graph, Spec, Tree, eval};
use track_record_server::db::TokenScope;
use track_record_server::error::ApiError;
use track_record_server::service::catalog;
use track_record_server::service::charts::source::{self, Budget, Limits, Loaded};
use uuid::Uuid;

use common::*;

fn node(v: Value) -> SourceNode {
    serde_json::from_value(v).unwrap()
}

async fn load_with(app: &TestApp, v: Value, limits: Limits) -> Result<Loaded, ApiError> {
    let mut budget = Budget::new(limits.max_points);
    source::load(
        &app.pool,
        "runs",
        &node(v),
        &limits,
        &mut budget,
        &mut |_| {},
    )
    .await
}

async fn load(app: &TestApp, v: Value) -> Loaded {
    load_with(app, v, Limits::default()).await.unwrap()
}

fn frames(tree: &Tree) -> &[Frame] {
    match tree {
        Tree::Frames(f) => f,
        Tree::Groups(_) => panic!("a source gives a flat list"),
    }
}

fn attr<'a>(f: &'a Frame, name: &str) -> &'a AttrValue {
    &f.attrs
        .get(name)
        .unwrap_or_else(|| panic!("no {name}"))
        .value
}

fn text(s: &str) -> AttrValue {
    AttrValue::Text(s.into())
}

fn ys(frames: &[Frame]) -> Vec<Option<f64>> {
    frames
        .iter()
        .flat_map(|f| f.column_values("y").unwrap().to_vec())
        .collect()
}

fn s(repetition: i64, load_time: f64, warmup: bool) -> Value {
    let mut v = sample(repetition, load_time);
    v["is_warmup"] = json!(warmup);
    v
}

async fn new_run(api: &Api, app: &TestApp, started_at: &str) -> Uuid {
    let id = Uuid::new_v4();
    let (status, b) = api
        .put(&app.m(&format!("/runs/{id}")), &run_body(started_at))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{b}");
    id
}

async fn add_ce(api: &Api, app: &TestApp, run: Uuid, body: &Value) -> Uuid {
    let id = Uuid::new_v4();
    let (status, b) = api
        .put(&app.m(&format!("/runs/{run}/case-executions/{id}")), body)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{b}");
    id
}

#[tokio::test]
async fn a_source_gives_one_frame_per_sample_with_its_attributes() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let ui = app.ui_client();
    let (run, ce) = seed_run(&write, &app, "pixel-8-a", "2026-11-01T10:00:00Z").await;
    for alias in [
        json!({"dimension": "client_device", "source": "pixel-8-a", "target": "pixel-8"}),
        json!({"dimension": "parameter:size_mb", "source": 10, "target": 12}),
    ] {
        let (status, b) = ui.put(&app.u("/aliases"), &alias).await;
        assert_eq!(status, StatusCode::OK, "{b}");
    }

    let out = load(&app, json!({"metric": "load_time", "unit": "ms"})).await;
    assert_eq!(out.samples, 2);
    let fs = frames(&out.tree);
    assert_eq!(fs.len(), 2);
    assert_eq!(ys(fs), [Some(1500.0), Some(1250.0)]);
    let f = &fs[0];
    assert_eq!(f.columns.len(), 1);
    assert_eq!(
        (f.columns[0].name.as_str(), f.columns[0].unit.as_str()),
        ("y", "ms")
    );

    // Every attribute of the sample, aliases applied.
    assert_eq!(attr(f, "run.id"), &text(&run.to_string()));
    assert!(matches!(attr(f, "run.started_at"), AttrValue::Time(_)));
    assert_eq!(attr(f, "run.status"), &text("running"));
    assert_eq!(attr(f, "run.trigger"), &text("nightly"));
    assert_eq!(attr(f, "run.valid"), &AttrValue::Bool(true));
    assert_eq!(attr(f, "run.tags"), &AttrValue::List(vec![text("nightly")]));
    assert!(
        !f.attrs.contains_key("run.ended_at"),
        "a running run has no end"
    );
    assert_eq!(attr(f, "suite.name"), &text("nightly"));
    assert_eq!(attr(f, "suite.version"), &AttrValue::Number(1.into()));
    assert_eq!(attr(f, "sut.commit"), &text("abc123"));
    assert_eq!(attr(f, "sut.branch"), &text("main"));
    assert!(!f.attrs.contains_key("sut.tag"));
    assert!(matches!(attr(f, "sut.id"), AttrValue::Number(_)));
    assert!(matches!(attr(f, "environment.id"), AttrValue::Number(_)));
    assert_eq!(attr(f, "scenario.name"), &text("load-model"));
    assert_eq!(attr(f, "scenario.version"), &AttrValue::Number(1.into()));
    assert_eq!(attr(f, "asset.engine.glb"), &text("v1"));
    assert_eq!(attr(f, "param.pipeline"), &text("classic"));
    // Parameters keep their JSON type and unit after aliases.
    assert_eq!(attr(f, "param.size_mb"), &AttrValue::Number(12.into()));
    assert_eq!(f.attrs["param.size_mb"].unit, "MB");
    assert_eq!(attr(f, "device.name"), &text("pixel-8"));
    assert!(matches!(attr(f, "device.details"), AttrValue::Number(_)));
    assert_eq!(attr(f, "case_execution.id"), &text(&ce.to_string()));
    assert_eq!(attr(f, "sample.repetition"), &AttrValue::Number(0.into()));
    assert_eq!(
        attr(&fs[1], "sample.repetition"),
        &AttrValue::Number(1.into())
    );
    assert_eq!(attr(f, "sample.warmup"), &AttrValue::Bool(false));
    assert_eq!(attr(f, "source"), &text("runs"));
    assert_eq!(attr(f, "metric"), &text("load_time"));

    // A series: x and y, x in the source's x unit.
    let out = load(&app, json!({"metric": "series.fps", "x_unit": "ms"})).await;
    let f = &frames(&out.tree)[0];
    let cols: Vec<(&str, &str)> = f
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c.unit.as_str()))
        .collect();
    assert_eq!(cols, [("x", "ms"), ("y", "fps")]);
    assert_eq!(f.columns[0].dimension, "time");
    assert_eq!(
        f.column_values("x").unwrap(),
        [
            Some(0.0),
            Some(500.0),
            Some(1000.0),
            Some(1000.0),
            Some(2000.0)
        ]
    );

    // A scalar of a series.
    let out = load(&app, json!({"metric": "max(series.fps)"})).await;
    assert_eq!(ys(frames(&out.tree)), [Some(60.0), Some(60.0)]);
}

#[tokio::test]
async fn a_source_leaves_out_invalid_runs_and_warmup_samples_unless_asked() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let a = new_run(&write, &app, "2026-11-01T10:00:00Z").await;
    let body = case_execution_body(
        "pixel-8",
        10,
        json!([s(0, 100.0, true), s(1, 1.0, false), s(2, 2.0, false)]),
    );
    add_ce(&write, &app, a, &body).await;
    let b = new_run(&write, &app, "2026-11-02T10:00:00Z").await;
    let body = case_execution_body("pixel-8", 10, json!([s(0, 4.0, false)]));
    add_ce(&write, &app, b, &body).await;
    let (status, _) = app
        .ui_client()
        .post(
            &app.u(&format!("/runs/{b}/invalidate")),
            &json!({"reason": "broken"}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let out = load(&app, json!({"metric": "load_time"})).await;
    assert_eq!(ys(frames(&out.tree)), [Some(1.0), Some(2.0)]);
    let out = load(&app, json!({"metric": "load_time", "include_warmup": true})).await;
    assert_eq!(ys(frames(&out.tree)), [Some(100.0), Some(1.0), Some(2.0)]);
    assert_eq!(
        attr(&frames(&out.tree)[0], "sample.warmup"),
        &AttrValue::Bool(true)
    );
    let out = load(
        &app,
        json!({"metric": "load_time", "filter": {"validity": "all"}}),
    )
    .await;
    let fs = frames(&out.tree);
    // Ordered by measured start of the run.
    assert_eq!(ys(fs), [Some(1.0), Some(2.0), Some(4.0)]);
    assert_eq!(attr(&fs[2], "run.valid"), &AttrValue::Bool(false));

    // The relative selection of runs (FR-11) applies.
    let out = load(
        &app,
        json!({"metric": "load_time", "filter": {"validity": "all", "latest_runs": 1}}),
    )
    .await;
    assert_eq!(ys(frames(&out.tree)), [Some(4.0)]);
    let out = load(
        &app,
        json!({"metric": "load_time", "filter": {"run_id": [a.to_string()]}}),
    )
    .await;
    assert_eq!(frames(&out.tree).len(), 2);
}

#[tokio::test]
async fn a_source_never_uses_superseded_case_executions() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let (run, ce) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let mut correction = case_execution_body("pixel-8", 10, json!([sample(0, 1.4)]));
    correction["supersedes_id"] = json!(ce);
    let fixed = add_ce(&write, &app, run, &correction).await;

    let out = load(&app, json!({"metric": "load_time"})).await;
    let fs = frames(&out.tree);
    assert_eq!(ys(fs), [Some(1.4)]);
    assert_eq!(attr(&fs[0], "case_execution.id"), &text(&fixed.to_string()));
}

#[tokio::test]
async fn samples_the_metric_does_not_apply_to_are_excluded() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;

    // Not measured: no frame, counted as missing.
    let out = load(&app, json!({"metric": "frames"})).await;
    assert!(frames(&out.tree).is_empty());
    assert_eq!((out.excluded.missing, out.excluded.errors), (2, 0));

    // A derived scalar without a value is a point with no value.
    let out = load(&app, json!({"metric": "time_to(series.fps, 1000)"})).await;
    assert_eq!(ys(frames(&out.tree)), [None, None]);
    assert_eq!(out.excluded.missing, 0);

    // A metric expression does not read attributes.
    let err = load_with(
        &app,
        json!({"metric": "param.size_mb * 2"}),
        Limits::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "invalid_expression");
    assert!(err.to_string().contains("unknown prefix"), "{err}");
    assert!(err.to_string().starts_with("source runs:"), "{err}");
}

#[tokio::test]
async fn a_source_stops_at_its_limits() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;

    let samples = Limits {
        max_samples: 1,
        ..Limits::default()
    };
    let err = load_with(&app, json!({"metric": "load_time"}), samples)
        .await
        .unwrap_err();
    assert_eq!(err.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(err.to_string().contains("more than 1 samples"), "{err}");

    // The points of all sources count against one budget.
    let points = Limits {
        max_points: 12,
        ..Limits::default()
    };
    let mut budget = Budget::new(points.max_points);
    let fps = node(json!({"metric": "series.fps"}));
    source::load(&app.pool, "a", &fps, &points, &mut budget, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(budget.used(), 10);
    let err = source::load(&app.pool, "b", &fps, &points, &mut budget, &mut |_| {})
        .await
        .unwrap_err();
    // The second source fails before it loads inputs that do not fit
    // into what the first one left.
    assert!(
        err.to_string()
            .contains("more than the 2 points the chart may still hold"),
        "{err}"
    );
    assert!(err.to_string().contains("max(series.fps)"), "{err}");
    // Without series inputs, the frames made are what counts.
    let mut budget = Budget::new(1);
    let err = source::load(
        &app.pool,
        "d",
        &node(json!({"metric": "load_time"})),
        &points,
        &mut budget,
        &mut |_| {},
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("more than 1 points"), "{err}");
    // A sample whose inputs alone exceed the points a run may hold fails
    // before its series are loaded, also for a scalar of them.
    let tiny = Limits {
        max_points: 4,
        ..Limits::default()
    };
    let err = load_with(&app, json!({"metric": "max(series.fps)"}), tiny)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("has 5 points"), "{err}");

    let scalars = node(json!({"metric": "max(series.fps)"}));
    let mut budget = Budget::new(points.max_points);
    let mut seen = Vec::new();
    source::load(&app.pool, "c", &scalars, &points, &mut budget, &mut |n| {
        seen.push(n)
    })
    .await
    .unwrap();
    assert_eq!((budget.used(), seen), (2, vec![2]));
}

#[tokio::test]
async fn a_graph_runs_on_the_frames_of_its_sources() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let r1 = new_run(&write, &app, "2026-11-01T10:00:00Z").await;
    let r2 = new_run(&write, &app, "2026-11-02T10:00:00Z").await;
    for (run, device, ys) in [
        (r1, "pixel-8", [1.0, 2.0]),
        (r1, "iphone", [5.0, 6.0]),
        (r2, "pixel-8", [3.0, 4.0]),
    ] {
        let body = case_execution_body(device, 10, json!([sample(0, ys[0]), sample(1, ys[1])]));
        add_ce(&write, &app, run, &body).await;
    }

    // The trend of ADR 0022 up to the samples per run, case and device.
    let spec = Spec::from_json(&json!({
        "format": 1,
        "variables": [{"name": "branch", "type": "text", "default": "main"}],
        "nodes": [
            {"id": "runs", "type": "source", "metric": "load_time",
             "filter": {"suite": "nightly", "sut_branch": {"var": "branch"}}},
            {"id": "lines", "type": "group", "input": "runs", "by": ["case", "device.name"]},
            {"id": "per_run", "type": "group", "input": "lines", "by": ["run.id"]},
            {"id": "samples", "type": "concatenate", "input": "per_run"},
            // Runs are grouped in the order of their ids; sort them by time.
            {"id": "ordered", "type": "sort", "input": "samples", "by": [{"key": "run.started_at"}]},
            {"id": "chart", "type": "table", "input": "ordered", "columns": ["y"]}
        ]
    }))
    .unwrap()
    .bind(&BTreeMap::new())
    .unwrap();
    let graph = Graph::new(&spec).unwrap();
    let NodeKind::Source(src) = &spec.node("runs").unwrap().kind else {
        panic!("a source")
    };
    let limits = Limits::default();
    let mut budget = Budget::new(limits.max_points);
    let loaded = source::load(&app.pool, "runs", src, &limits, &mut budget, &mut |_| {})
        .await
        .unwrap();
    let sources = BTreeMap::from([("runs".to_owned(), loaded.tree)]);
    let catalogue = catalog::unit_catalogue(&app.pool).await.unwrap();
    let out = eval::run(&spec, &graph, sources, None, &catalogue).unwrap();

    // One line per case and device, ordered by key; one frame per run with
    // a point per sample, which keeps the attributes of its sample.
    let Tree::Groups(lines) = &out else {
        panic!("one level of lines")
    };
    let devices: Vec<&AttrValue> = lines
        .iter()
        .map(|g| {
            g.key
                .iter()
                .find(|(n, _)| n == "device.name")
                .and_then(|(_, v)| v.as_ref())
                .map(|a| &a.value)
                .unwrap()
        })
        .collect();
    assert_eq!(devices, [&text("iphone"), &text("pixel-8")]);
    let pixel = frames(&lines[1].tree);
    assert_eq!(ys(pixel), [Some(1.0), Some(2.0), Some(3.0), Some(4.0)]);
    assert_eq!(attr(&pixel[1], "run.id"), &text(&r2.to_string()));
    let rep = |f: &Frame, i| f.point_attr(i, "sample.repetition").unwrap().value.clone();
    assert_eq!(
        (rep(&pixel[0], 0), rep(&pixel[0], 1)),
        (AttrValue::Number(0.into()), AttrValue::Number(1.into()))
    );
    assert!(
        !pixel[0].attrs.contains_key("sample.id"),
        "differs per point"
    );
}

#[tokio::test]
async fn assets_of_the_same_name_stay_apart() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let run = new_run(&write, &app, "2026-11-01T10:00:00Z").await;
    for (assets, y) in [
        (
            json!([{"name": "model", "version": "v1"}, {"name": "model", "version": "v2"}]),
            1.0,
        ),
        (json!([{"name": "model", "version": "v2"}]), 2.0),
        (json!([{"name": "model", "content_hash": "v2"}]), 3.0),
        (json!([{"name": "model", "version": "v1 (abc)"}]), 4.0),
        (
            json!([{"name": "model", "version": "v1", "content_hash": "abc"}]),
            5.0,
        ),
    ] {
        let mut body = case_execution_body("pixel-8", 10, json!([sample(0, y)]));
        body["test_case"]["assets"] = assets;
        add_ce(&write, &app, run, &body).await;
    }
    let out = load(&app, json!({"metric": "load_time"})).await;
    let mut seen: Vec<(Option<f64>, AttrValue)> = frames(&out.tree)
        .iter()
        .map(|f| {
            (
                f.column_values("y").unwrap()[0],
                attr(f, "asset.model").clone(),
            )
        })
        .collect();
    seen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let list = |parts: &[&str]| AttrValue::List(parts.iter().map(|p| text(p)).collect());
    assert_eq!(
        seen,
        [
            (
                Some(1.0),
                AttrValue::List(vec![list(&["version=v1"]), list(&["version=v2"])])
            ),
            (Some(2.0), text("v2")),
            (Some(3.0), list(&["hash=v2"])),
            (Some(4.0), text("v1 (abc)")),
            (Some(5.0), list(&["version=v1", "hash=abc"])),
        ]
    );
}

#[tokio::test]
async fn long_series_are_loaded_in_batches_with_the_same_result() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let (run, _) = seed_run(&write, &app, "pixel-8", "2026-11-01T10:00:00Z").await;
    let body = case_execution_body("iphone", 10, json!([sample(0, 3.0)]));
    add_ce(&write, &app, run, &body).await;

    let all = load(&app, json!({"metric": "series.fps"})).await;
    // Every sample has a series of 5 points: one sample per batch.
    let small = Limits {
        batch_points: 6,
        ..Limits::default()
    };
    let batched = load_with(&app, json!({"metric": "series.fps"}), small)
        .await
        .unwrap();
    assert_eq!(frames(&batched.tree).len(), 3);
    assert_eq!(batched.tree, all.tree);
    let scalars = load_with(&app, json!({"metric": "max(series.fps)"}), small)
        .await
        .unwrap();
    assert_eq!(ys(frames(&scalars.tree)), [Some(60.0); 3]);
}

#[tokio::test]
async fn a_series_without_a_value_needs_a_fitting_x_too() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let run = new_run(&write, &app, "2026-11-01T10:00:00Z").await;
    for x_unit in ["s", "B"] {
        let mut one = sample(0, 1.0);
        one["series"][0]["x_unit"] = json!(x_unit);
        let body = case_execution_body("pixel-8", 10, json!([one]));
        add_ce(&write, &app, run, &body).await;
    }
    // The latest upload has x in B; the sample with x in s does not fit.
    let out = load(&app, json!({"metric": "series.fps * (1 / 0)"})).await;
    let fs = frames(&out.tree);
    assert_eq!(fs.len(), 1, "{fs:#?}");
    assert!(fs[0].is_empty(), "no value: a frame without points");
    assert_eq!(fs[0].columns[0].unit, "B");
    assert_eq!(out.excluded.errors, 1);
}

#[tokio::test]
async fn an_x_that_overflows_in_the_x_unit_is_excluded() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let run = new_run(&write, &app, "2026-11-01T10:00:00Z").await;
    for (size, scale) in [(10, 1e-30), (20, 1.0)] {
        let mut one = sample(0, 1.0);
        let y: Vec<f64> = [10.0, 30.0, 60.0, 55.0, 58.0]
            .iter()
            .map(|y| y * scale)
            .collect();
        one["series"][0]["y"] = json!(y);
        let body = case_execution_body("pixel-8", size, json!([one]));
        add_ce(&write, &app, run, &body).await;
    }
    // x of about 1e302 s is no finite number of ns; x of about 1e272 s is.
    let metric = "rekey(series.fps, cumsum(series.fps) * 1e300[s/fps])";
    let out = load(&app, json!({"metric": metric, "x_unit": "ns"})).await;
    let fs = frames(&out.tree);
    assert_eq!(fs.len(), 1, "{fs:#?}");
    assert_eq!(fs[0].values[0].len(), 5);
    assert!(fs[0].values[0].iter().flatten().all(|x| x.is_finite()));
    assert_eq!(out.excluded.errors, 1);
}

#[tokio::test]
async fn a_sample_without_the_metric_needs_no_room() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let run = new_run(&write, &app, "2026-11-01T10:00:00Z").await;
    // The second sample lacks load_time; the first fills the budget.
    let mut lacking = sample(1, 1.0);
    lacking["measurements"] = json!({});
    let body = case_execution_body("pixel-8", 10, json!([sample(0, 1.0), lacking]));
    add_ce(&write, &app, run, &body).await;
    let limits = Limits::default();
    let mut budget = Budget::new(1);
    let out = source::load(
        &app.pool,
        "a",
        &node(json!({"metric": "load_time"})),
        &limits,
        &mut budget,
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(frames(&out.tree).len(), 1);
    assert_eq!(out.excluded.missing, 1);
    assert_eq!(budget.used(), 1);
}
