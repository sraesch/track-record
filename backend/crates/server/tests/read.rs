//! Reading: run filters (FR-11), aliases (FR-14), pagination and
//! definition lists.

mod common;

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use reqwest::StatusCode;
use serde_json::{Value, json};
use track_record_server::db::TokenScope;
use uuid::Uuid;

use common::*;

fn ids(list: &Value) -> Vec<String> {
    list["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn run_filters() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let read = app.as_scope(TokenScope::Read).await;
    let (a, _) = seed_run(&write, &app, "pixel-8-a", "2026-11-01T10:00:00Z").await;
    let (b, _) = seed_run(&write, &app, "iphone-15", "2026-11-02T10:00:00Z").await;

    // A third run with another SUT version, parameter value and a tag.
    let c = Uuid::new_v4();
    let mut body = run_body("2026-11-03T10:00:00Z");
    body["sut_version"]["commit"] = json!("fff000");
    body["tags"] = json!(["nightly", "experiment"]);
    body["triggered_by"] = json!("ci");
    assert_eq!(
        write.put(&app.m(&format!("/runs/{c}")), &body).await.0,
        StatusCode::CREATED
    );
    let mut ce = case_execution_body("pixel-8-b", 20, json!([]));
    ce["test_case"]["parameters"]["pipeline"] = json!("new");
    ce["metrics"] = json!({});
    assert_eq!(
        write
            .put(
                &app.m(&format!("/runs/{c}/case-executions/{}", Uuid::new_v4())),
                &ce
            )
            .await
            .0,
        StatusCode::CREATED
    );
    write
        .post(
            &app.m(&format!("/runs/{a}/close")),
            &json!({"status": "completed"}),
        )
        .await;

    let q = |query: &str| {
        let read = read.clone();
        let query = query.to_owned();
        let url = app.m(&format!("/runs?{query}"));
        async move {
            let (status, list) = read.get(&url).await;
            assert_eq!(status, StatusCode::OK, "{query}: {list}");
            ids(&list)
        }
    };
    let (a, b, c) = (a.to_string(), b.to_string(), c.to_string());

    assert_eq!(
        q("").await,
        vec![c.clone(), b.clone(), a.clone()],
        "newest measured start first"
    );
    assert_eq!(q("sut_commit=fff000").await, vec![c.clone()]);
    assert_eq!(q("suite=nightly&sut_branch=main").await.len(), 3);
    assert_eq!(q("suite=other").await.len(), 0);
    assert_eq!(
        q("started_after=2026-11-02T00:00:00Z").await,
        vec![c.clone(), b.clone()]
    );
    assert_eq!(
        q("started_before=2026-11-02T00:00:00Z").await,
        vec![a.clone()]
    );
    assert_eq!(q("status=completed").await, vec![a.clone()]);
    assert_eq!(q("status=running&status=completed").await.len(), 3);
    assert_eq!(q("triggered_by=ci").await, vec![c.clone()]);
    assert_eq!(q("tag=experiment").await, vec![c.clone()]);
    assert_eq!(q("tag=experiment&tag=nightly").await, vec![c.clone()]);
    assert_eq!(q("tag=nightly").await.len(), 3);
    assert_eq!(q("client_device=iphone-15").await, vec![b.clone()]);
    assert_eq!(q("scenario=load-model").await.len(), 3);
    assert_eq!(q("asset=engine.glb").await.len(), 3);
    assert_eq!(q("asset=other.glb").await.len(), 0);
    assert_eq!(q("param=size_mb=20").await, vec![c.clone()]);
    assert_eq!(
        q("param=size_mb=%2220%22").await.len(),
        0,
        "\"20\" is a string, not 20"
    );
    assert_eq!(
        q("param=pipeline=classic").await,
        vec![b.clone(), a.clone()]
    );
    assert_eq!(
        q("param=pipeline=new&client_device=pixel-8-b").await,
        vec![c.clone()]
    );
    assert_eq!(
        q("param=pipeline=new&client_device=pixel-8-a").await.len(),
        0
    );
    assert_eq!(q("metric=fps").await, vec![b.clone(), a.clone()]);
    assert_eq!(q("metric=load_time").await, vec![b.clone(), a.clone()]);
    assert_eq!(q("metric=unknown").await.len(), 0);

    let shared_env = {
        let (_, run) = read.get(&app.m(&format!("/runs/{a}"))).await;
        run["shared_environment_id"].as_i64().unwrap()
    };
    assert_eq!(
        q(&format!("shared_environment_id={shared_env}"))
            .await
            .len(),
        3
    );

    // Bad input.
    assert_eq!(
        read.get(&app.m("/runs?limit=0")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read.get(&app.m("/runs?cursor=zzz")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read.get(&app.m("/runs?param=nameonly")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read.get(&app.m("/runs?status=sleeping")).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read.get(&app.m("/runs?bogus=1")).await.0,
        StatusCode::BAD_REQUEST
    );

    // Aliases: both pixel names become one device, in filters and in output.
    let ui = app.ui_client();
    for source in ["pixel-8-a", "pixel-8-b"] {
        let (status, alias) = ui
            .put(
                &app.u("/aliases"),
                &json!({"dimension": "client_device", "source": source,
                                              "target": "pixel-8"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{alias}");
    }
    assert_eq!(q("client_device=pixel-8").await, vec![c.clone(), a.clone()]);
    assert_eq!(q("client_device=pixel-8-a").await.len(), 0, "aliased away");
    let (_, ces) = read
        .get(&app.m(&format!("/runs/{a}/case-executions")))
        .await;
    assert_eq!(ces[0]["client_device"]["name"], "pixel-8-a");
    assert_eq!(ces[0]["client_device"]["resolved_name"], "pixel-8");
    let (_, devices) = read.get(&app.m("/client-devices")).await;
    let resolved: Vec<&str> = devices
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["resolved_name"].as_str().unwrap())
        .collect();
    assert_eq!(resolved, vec!["iphone-15", "pixel-8", "pixel-8"]);

    // Parameter aliases keep their JSON type: "new" -> "next".
    ui.put(
        &app.u("/aliases"),
        &json!({"dimension": "parameter:pipeline", "source": "new",
                                         "target": "next"}),
    )
    .await;
    assert_eq!(q("param=pipeline=next").await, vec![c.clone()]);
    let (_, ces) = read
        .get(&app.m(&format!("/runs/{c}/case-executions")))
        .await;
    assert_eq!(ces[0]["test_case"]["parameters"]["pipeline"], "new");
    assert_eq!(
        ces[0]["test_case"]["resolved_parameters"]["pipeline"],
        "next"
    );

    // Removing an alias is a new version; the old name matches again.
    let (_, removed) = ui
        .put(
            &app.u("/aliases"),
            &json!({"dimension": "client_device", "source": "pixel-8-a",
                                          "target": null}),
        )
        .await;
    assert_eq!(removed["version"], 2);
    assert!(removed["target"].is_null());
    assert_eq!(q("client_device=pixel-8-a").await, vec![a.clone()]);
    let (_, aliases) = read.get(&app.m("/aliases")).await;
    assert_eq!(aliases.as_array().unwrap().len(), 3);
    // Invalid aliases.
    let (status, _) = ui
        .put(
            &app.u("/aliases"),
            &json!({"dimension": "client_device", "source": "x", "target": "x"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

/// A measured start `days` before now, in whole seconds.
fn days_ago(now: DateTime<Utc>, days: i64) -> String {
    (now - TimeDelta::days(days)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Creates a run with one case execution on `device`.
async fn marked_run(
    write: &Api,
    app: &TestApp,
    started_at: &str,
    device: &str,
    sut: Value,
    tags: Value,
) -> String {
    let id = Uuid::new_v4();
    let mut body = run_body(started_at);
    body["sut_version"] = sut;
    body["tags"] = tags;
    let (status, out) = write.put(&app.m(&format!("/runs/{id}")), &body).await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    let (status, out) = write
        .put(
            &app.m(&format!("/runs/{id}/case-executions/{}", Uuid::new_v4())),
            &case_execution_body(device, 10, json!([sample(0, 1.5)])),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    id.to_string()
}

#[tokio::test]
async fn relative_run_selection() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let now = Utc::now();
    let main = json!({"commit": "abc123", "branch": "main", "build_config": "release"});
    // A release run on its own branch, uploaded now with an old measured start.
    let a = marked_run(
        &write,
        &app,
        &days_ago(now, 40),
        "pixel-8",
        json!({"commit": "rel100", "branch": "release", "build_config": "release",
               "tag": "v1.0.0"}),
        json!(["baseline"]),
    )
    .await;
    // A second, later run with the same run tag.
    let b = marked_run(
        &write,
        &app,
        &days_ago(now, 10),
        "pixel-8",
        main.clone(),
        json!(["baseline"]),
    )
    .await;
    let c = marked_run(
        &write,
        &app,
        &days_ago(now, 2),
        "pixel-8",
        main.clone(),
        json!([]),
    )
    .await;
    // Two runs with the same start: ordered by id, newest (largest) first.
    let same = days_ago(now, 1);
    let mut de = vec![
        marked_run(&write, &app, &same, "iphone-15", main.clone(), json!([])).await,
        marked_run(&write, &app, &same, "iphone-15", main.clone(), json!([])).await,
    ];
    de.sort_by(|x, y| y.cmp(x));
    let newest = |rest: &[&String]| -> Vec<String> {
        de.iter().chain(rest.iter().copied()).cloned().collect()
    };

    let read = app.as_scope(TokenScope::Read).await;
    let ports = [(read, app.m("/runs")), (app.ui_client(), app.u("/runs"))];
    for (api, base) in &ports {
        let q = |query: String| async move {
            let (status, list) = api.get(&format!("{base}?{query}")).await;
            assert_eq!(status, StatusCode::OK, "{base}?{query}: {list}");
            ids(&list)
        };
        assert_eq!(q(String::new()).await, newest(&[&c, &b, &a]));

        // Last N days, by measured start: the run uploaded now with an old
        // start is not among them.
        assert_eq!(q("started_within_days=7".into()).await, newest(&[&c]));
        assert_eq!(q("started_within_days=30".into()).await, newest(&[&c, &b]));

        // Latest N runs; ties by id; applied after every other condition.
        assert_eq!(q("latest_runs=3".into()).await, newest(&[&c]));
        assert_eq!(q("latest_runs=1".into()).await, vec![de[0].clone()]);
        assert_eq!(
            q("latest_runs=2&client_device=pixel-8".into()).await,
            vec![c.clone(), b.clone()]
        );
        assert_eq!(
            q("latest_runs=2&started_within_days=5".into()).await,
            de.clone()
        );

        // Since a marked run: the earliest of several, looked up among all runs.
        assert_eq!(q("since_tag=baseline".into()).await, newest(&[&c, &b, &a]));
        assert_eq!(
            q("since_sut_tag=v1.0.0&sut_branch=main".into()).await,
            newest(&[&c, &b]),
            "the release run is not on main but still marks the start"
        );

        // Combined: all conditions hold, the latest lower bound wins.
        assert_eq!(
            q("since_tag=baseline&started_within_days=30".into()).await,
            newest(&[&c, &b])
        );
        assert_eq!(
            q(format!(
                "since_sut_tag=v1.0.0&started_after={}",
                days_ago(now, 5)
            ))
            .await,
            newest(&[&c])
        );
        assert_eq!(
            q(
                "since_tag=baseline&started_within_days=30&client_device=pixel-8&latest_runs=1"
                    .into()
            )
            .await,
            vec![c.clone()]
        );

        // `limit` and `cursor` page through the selected runs.
        let (_, page) = api.get(&format!("{base}?latest_runs=3&limit=2")).await;
        assert_eq!(ids(&page), de.clone());
        let cursor = page["next_cursor"].as_str().expect("a second page");
        let (_, page) = api
            .get(&format!("{base}?latest_runs=3&limit=2&cursor={cursor}"))
            .await;
        assert_eq!(ids(&page), vec![c.clone()]);
        assert!(page["next_cursor"].is_null());

        // A mark that no run has is an error that names it.
        for query in ["since_tag=nope", "since_sut_tag=nope"] {
            let (status, problem) = api.get(&format!("{base}?{query}")).await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
            assert_eq!(problem["code"], "validation_failed");
            assert!(
                problem["detail"].as_str().unwrap().contains("\"nope\""),
                "{problem}"
            );
        }
        for query in ["latest_runs=0", "started_within_days=0", "latest_runs=-1"] {
            let (status, _) = api.get(&format!("{base}?{query}")).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        }
    }
}

#[tokio::test]
async fn pagination() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    let mut expected = Vec::new();
    for day in 1..=5 {
        let (id, _) = seed_run(&write, &app, "dev", &format!("2026-11-0{day}T10:00:00Z")).await;
        expected.push(id.to_string());
    }
    expected.reverse();

    let mut seen = Vec::new();
    let mut url = app.m("/runs?limit=2");
    loop {
        let (status, page) = write.get(&url).await;
        assert_eq!(status, StatusCode::OK);
        seen.extend(ids(&page));
        match page["next_cursor"].as_str() {
            Some(cursor) => url = app.m(&format!("/runs?limit=2&cursor={cursor}")),
            None => break,
        }
    }
    assert_eq!(seen, expected);
}

#[tokio::test]
async fn definitions_are_listed_with_versions() {
    let app = app!();
    let write = app.as_scope(TokenScope::Write).await;
    seed_run(&write, &app, "dev", "2026-11-01T10:00:00Z").await;
    // A changed scenario definition is a new scenario version and test case.
    let (run_id, _) = seed_run(&write, &app, "dev", "2026-11-02T10:00:00Z").await;
    let mut ce = case_execution_body("dev", 10, json!([]));
    ce["test_case"]["scenario"]["definition"] = json!({"steps": ["open", "render", "close"]});
    // Metrics have no versions: an upload does not change one that exists.
    ce["metrics"]["load_time"]["description"] = json!("time to first frame");
    ce["samples"] = json!([sample(0, 1.0)]);
    let (status, body) = write
        .put(
            &app.m(&format!(
                "/runs/{run_id}/case-executions/{}",
                Uuid::new_v4()
            )),
            &ce,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (_, scenarios) = write.get(&app.m("/scenarios")).await;
    assert_eq!(scenarios[0]["name"], "load-model");
    assert_eq!(scenarios[0]["versions"].as_array().unwrap().len(), 2);
    let (_, cases) = write.get(&app.m("/test-cases")).await;
    assert_eq!(cases.as_array().unwrap().len(), 2);
    let (_, metrics) = write.get(&app.m("/metrics")).await;
    let load = metrics
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "load_time")
        .unwrap();
    assert_eq!(
        (&load["unit"], &load["description"], &load["has_results"]),
        (&json!("s"), &json!(""), &json!(true))
    );
    assert_eq!(
        (&load["version"], &load["versions"]),
        (&Value::Null, &Value::Null)
    );
    let (_, suites) = write.get(&app.m("/suites")).await;
    assert_eq!(suites[0]["versions"].as_array().unwrap().len(), 1);

    let (_, units) = write.get(&app.m("/units")).await;
    assert!(
        units
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["symbol"] == "MiB/s")
    );
    let (_, assets) = write.get(&app.m("/assets")).await;
    assert_eq!(assets.as_array().unwrap().len(), 1);
    let (_, sut) = write.get(&app.m("/sut-versions")).await;
    assert_eq!(sut.as_array().unwrap().len(), 1);

    let (_, run) = write.get(&app.m(&format!("/runs/{run_id}"))).await;
    let env_id = run["shared_environment_id"].as_i64().unwrap();
    let (status, env) = write.get(&app.m(&format!("/environments/{env_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(env["details"], json!({"server": "backend-1"}));
    assert_eq!(env["fingerprint"].as_str().unwrap().len(), 64);
    let tc_id = cases[0]["id"].as_i64().unwrap();
    assert_eq!(
        write.get(&app.m(&format!("/test-cases/{tc_id}"))).await.0,
        StatusCode::OK
    );
    assert_eq!(
        write.get(&app.m("/test-cases/999999")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        write.get(&app.m("/environments/999999")).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn missing_things_are_not_found_as_problem_json() {
    let app = app!();
    let read = app.as_scope(TokenScope::Read).await;
    for path in [
        format!("/runs/{}", Uuid::new_v4()),
        format!("/runs/{}/case-executions", Uuid::new_v4()),
        format!("/case-executions/{}", Uuid::new_v4()),
        format!("/series/{}", Uuid::new_v4()),
        format!("/series/{}/points", Uuid::new_v4()),
        format!("/attachments/{}", Uuid::new_v4()),
        "/no-such-endpoint".to_owned(),
    ] {
        let response = read.raw(app.client.get(app.m(&path))).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json",
            "{path}"
        );
    }
    let (status, problem) = read.get(&app.m("/runs/not-a-uuid")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem["code"], "invalid_path");
}
