//! Tests of reading, checking and binding whole specs.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;
use crate::dsl::check::tests::{TestEnv, env};
use crate::units::tests::catalogue;

fn test_env() -> TestEnv {
    let mut e = env();
    e.metrics.insert("load_time".into(), "ms".into());
    e
}

/// The spec example of ADR 0022: a trend.
fn adr_example() -> Value {
    json!({
      "format": 1,
      "variables": [{"name": "branch", "type": "text", "default": "main"}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time",
         "filter": {"suite": "web-viewer", "sut_branch": {"var": "branch"}}},
        {"id": "lines", "type": "group", "input": "runs",
         "by": ["case", "device.name"]},
        {"id": "per_run", "type": "group", "input": "lines", "by": ["run.id"]},
        {"id": "samples", "type": "concatenate", "input": "per_run"},
        {"id": "stats", "type": "reduce", "input": "samples",
         "functions": ["mean", "ci_low", "ci_high"]},
        {"id": "chart", "type": "line", "input": "stats",
         "x": "run.started_at", "y": "mean", "low": "ci_low", "high": "ci_high"}
      ]
    })
}

/// A spec with a source of `load_time` and the given nodes, which read
/// `runs` or each other; `chart` is a table of `y` unless given.
fn spec_with(nodes: Value) -> Value {
    let mut all = vec![json!({"id": "runs", "type": "source", "metric": "load_time"})];
    all.extend(nodes.as_array().unwrap().iter().cloned());
    if !all.iter().any(|n| n["id"] == "chart") {
        let last = all.last().unwrap()["id"].clone();
        all.push(json!({"id": "chart", "type": "table", "input": last, "columns": ["y"]}));
    }
    json!({"format": 1, "nodes": all})
}

/// The problems that keep a spec from being well-formed.
fn malformed(value: Value) -> Vec<Problem> {
    match Spec::from_json(&value) {
        Err(problems) => problems,
        Ok(spec) => match Graph::new(&spec) {
            Err(problems) => problems,
            Ok(_) => Vec::new(),
        },
    }
}

/// Validation of a well-formed spec.
fn validation(value: Value) -> Validation {
    let spec = Spec::from_json(&value).unwrap_or_else(|p| panic!("{p:?}"));
    let graph = Graph::new(&spec).unwrap_or_else(|p| panic!("{p:?}"));
    validate(&spec, &graph, &mut test_env(), &catalogue())
}

fn problems(value: Value) -> Vec<Problem> {
    validation(value).problems
}

/// Asserts a problem at `node` and `setting` whose message contains
/// `text`.
#[track_caller]
fn assert_problem(problems: &[Problem], node: Option<&str>, setting: Option<&str>, text: &str) {
    assert!(
        problems.iter().any(|p| p.node.as_deref() == node
            && p.setting.as_deref() == setting
            && p.message.contains(text)),
        "no problem at {node:?} {setting:?} with {text:?} in {problems:#?}"
    );
}

fn cols(shape: &Shape) -> Vec<(&str, Option<&str>)> {
    shape
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c.unit.as_deref()))
        .collect()
}

// The spec example ---------------------------------------------------------------------

#[test]
fn the_adr_example_is_well_formed_and_valid() {
    let value = adr_example();
    let spec = Spec::from_json(&value).unwrap();
    assert_eq!(spec.to_json(), value, "a spec round-trips through JSON");
    let graph = Graph::new(&spec).unwrap();
    assert_eq!(spec.nodes[graph.visualization()].id, "chart");
    let v = validate(&spec, &graph, &mut test_env(), &catalogue());
    assert!(v.problems.is_empty(), "{:#?}", v.problems);

    let depth = |id: &str| v.shapes[id].levels.len();
    assert_eq!(
        ["runs", "lines", "per_run", "samples", "stats", "chart"].map(depth),
        [0, 1, 2, 1, 0, 0]
    );
    assert_eq!(cols(&v.shapes["runs"]), [("y", Some("ms"))]);
    assert_eq!(cols(&v.shapes["samples"]), [("y", Some("ms"))]);
    assert_eq!(
        cols(&v.shapes["stats"]),
        [
            ("mean", Some("ms")),
            ("ci_low", Some("ms")),
            ("ci_high", Some("ms")),
            ("n", Some("")),
            ("n_no_value", Some(""))
        ]
    );
    assert_eq!(
        v.shapes["lines"].levels[0],
        Level::Group {
            by: vec!["case".into(), "device.name".into()],
            except: vec![]
        }
    );
    assert!(v.shapes["stats"].has_attribute("run.started_at"));
    assert!(v.shapes["stats"].has_attribute("param.width"));

    let report = check(&value, &mut test_env(), &catalogue());
    assert!(report.well_formed && report.valid);
    assert_eq!(report.nodes.len(), 6);
}

#[test]
fn a_series_source_has_x_and_y() {
    let v = validation(spec_with(json!([
        {"id": "fps", "type": "source", "metric": "series.fps", "x_unit": "ms"},
        {"id": "grid", "type": "resample", "input": "fps", "points": 50},
        {"id": "band", "type": "combine", "input": "grid",
         "functions": ["median", {"function": "percentile", "p": 10},
                       {"function": "percentile", "p": 90}, "count"]},
        {"id": "chart", "type": "line", "input": "band", "x": "x", "y": "median",
         "low": "p10", "high": "p90"},
    ])));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        cols(&v.shapes["fps"]),
        [("x", Some("ms")), ("y", Some("fps"))]
    );
    assert_eq!(
        cols(&v.shapes["band"]),
        [
            ("x", Some("ms")),
            ("median", Some("fps")),
            ("p10", Some("fps")),
            ("p90", Some("fps")),
            ("count", Some("")),
            ("n", Some("")),
            ("n_no_value", Some(""))
        ]
    );
}

#[test]
fn a_ratio_to_a_reference_goes_through_a_match() {
    let value = json!({
      "format": 1,
      "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time",
         "filter": {"run_id": ["7c6c5a3e-0c4b-4d4e-9c39-6f1e6c3b8a10"]}},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run", "sample"]},
        {"id": "ratio", "type": "combine", "input": "pairs",
         "expressions": [{"name": "ratio", "expression": "a / b", "unit": ""}]},
        {"id": "chart", "type": "table", "input": "ratio",
         "columns": ["case_execution.id", "ratio"], "differs": ["sut.id"]}
      ]
    });
    let v = validation(value);
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    let pairs = &v.shapes["pairs"];
    let Level::Match { inputs } = &pairs.levels[0] else {
        panic!("a match level")
    };
    assert_eq!(
        inputs.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    let ratio = &v.shapes["ratio"];
    assert!(ratio.levels.is_empty());
    assert_eq!(
        cols(ratio),
        [("a", Some("ms")), ("b", Some("ms")), ("ratio", Some(""))]
    );
    assert!(ratio.has_attribute("b.sut.id"));
    assert!(ratio.has_attribute("b.param.width"));
    assert!(ratio.has_attribute("sut.id"));
}

#[test]
fn side_by_side_columns_are_named_by_input_and_column() {
    let v = validation(json!({
      "format": 1,
      "nodes": [
        {"id": "a", "type": "source", "metric": "load_time"},
        {"id": "stats", "type": "reduce", "input": "a", "functions": ["mean"]},
        {"id": "b", "type": "source", "metric": "bytes"},
        {"id": "pairs", "type": "match", "inputs": {"a": "stats", "b": "b"}},
        {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
        {"id": "chart", "type": "scatter", "input": "both", "x": "a.mean", "y": "b"}
      ]
    }));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        cols(&v.shapes["both"]),
        [
            ("a.mean", Some("ms")),
            ("a.n", Some("")),
            ("a.n_no_value", Some("")),
            ("b", Some("B"))
        ]
    );
}

// Well-formed --------------------------------------------------------------------------

#[test]
fn the_format_and_the_settings_of_a_spec_are_checked() {
    let p = malformed(json!({"format": 2, "nodes": [], "title": "x"}));
    assert_problem(&p, None, Some("format"), "unsupported format 2");
    assert_problem(&p, None, Some("title"), "unknown setting");
    let p = malformed(json!({"nodes": []}));
    assert_problem(&p, None, Some("format"), "needs its format");
    let p = malformed(json!([1]));
    assert_problem(&p, None, None, "JSON object");
    let p = malformed(json!({"format": 1, "nodes": [{"type": "source"}]}));
    assert_problem(&p, None, Some("nodes[0].id"), "needs an id");
}

#[test]
fn unknown_types_and_settings_are_reported_per_node() {
    let p = malformed(spec_with(json!([
        {"id": "pie", "type": "pie", "input": "runs"},
        {"id": "g", "type": "group", "input": "runs", "byy": ["case"]},
        {"id": "r", "type": "reduce", "input": "runs", "functions": ["mode"]},
        {"id": "s", "type": "source", "metric": "bytes", "filter": {"branch": "main"}},
    ])));
    assert_problem(&p, Some("pie"), Some("type"), "unknown node type \"pie\"");
    assert_problem(&p, Some("g"), Some("byy"), "unknown field `byy`");
    assert_problem(&p, Some("r"), None, "unknown variant `mode`");
    assert_problem(&p, Some("s"), Some("filter.branch"), "no field \"branch\"");
    let p = malformed(spec_with(
        json!([{"id": "g", "type": "group", "input": "runs"}]),
    ));
    assert_problem(&p, Some("g"), Some("by"), "missing field `by`");
}

#[test]
fn ids_and_inputs_are_checked() {
    let p = malformed(spec_with(json!([
        {"id": "runs", "type": "source", "metric": "bytes"},
        {"id": "f", "type": "flatten"},
        {"id": "s", "type": "sort", "input": "nowhere", "by": [{"key": "run.id"}]},
        {"id": "src", "type": "source", "metric": "bytes", "input": "runs"},
        {"id": "g", "type": "group", "inputs": {"a": "runs"}, "input": "runs", "by": ["case"]},
    ])));
    assert_problem(&p, Some("runs"), Some("id"), "another node has the id");
    assert_problem(&p, Some("f"), Some("input"), "needs an input");
    assert_problem(&p, Some("s"), Some("input"), "there is no node \"nowhere\"");
    assert_problem(&p, Some("src"), Some("input"), "a source has no input");
    assert_problem(
        &p,
        Some("g"),
        Some("inputs"),
        "only a match has named inputs",
    );
}

#[test]
fn a_match_needs_named_inputs_with_a() {
    let p = malformed(spec_with(json!([
        {"id": "m1", "type": "match", "input": "runs"},
        {"id": "m2", "type": "match", "inputs": {"x": "runs", "y": "runs"}},
        {"id": "m3", "type": "match", "inputs": {"a": "runs"}},
        {"id": "m4", "type": "match", "inputs": {"a": "runs", "sut": "runs", "b-c": "runs"}},
    ])));
    assert_problem(&p, Some("m1"), Some("inputs"), "needs named inputs");
    assert_problem(&p, Some("m1"), Some("input"), "use inputs");
    assert_problem(&p, Some("m2"), Some("inputs"), "needs the input a");
    assert_problem(&p, Some("m3"), Some("inputs"), "at least two inputs");
    assert_problem(
        &p,
        Some("m4"),
        Some("inputs.sut"),
        "attributes start with it",
    );
    assert_problem(&p, Some("m4"), Some("inputs.b-c"), "not an input name");
}

#[test]
fn a_cycle_is_not_well_formed() {
    let p = malformed(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "bytes"},
        {"id": "a", "type": "filter", "input": "b", "conditions": []},
        {"id": "b", "type": "filter", "input": "a", "conditions": []},
        {"id": "chart", "type": "table", "input": "b", "columns": ["y"]}
    ]}));
    assert_eq!(p.len(), 1, "{p:#?}");
    assert!(p[0].message.contains("cycle: a → b → a") || p[0].message.contains("cycle: b → a → b"));
}

#[test]
fn a_chart_has_exactly_one_visualization_at_the_end() {
    let p = malformed(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "bytes"}
    ]}));
    assert_problem(&p, None, Some("nodes"), "needs a visualization");
    let p = malformed(spec_with(json!([
        {"id": "second", "type": "value", "input": "runs", "value": "y"},
    ])));
    assert_problem(&p, Some("chart"), None, "exactly one visualization");
    let p = malformed(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "bytes"},
        {"id": "chart", "type": "table", "input": "runs", "columns": ["y"]},
        {"id": "after", "type": "filter", "input": "chart", "conditions": []}
    ]}));
    assert_problem(&p, Some("after"), Some("input"), "is a visualization");
}

#[test]
fn a_graph_has_at_most_100_nodes_and_8_sources() {
    let mut nodes: Vec<Value> = (0..99)
        .map(|i| {
            let input = if i == 0 {
                "runs".to_owned()
            } else {
                format!("f{}", i - 1)
            };
            json!({"id": format!("f{i}"), "type": "filter", "input": input, "conditions": []})
        })
        .collect();
    let spec = spec_with(json!(nodes.clone()));
    assert_problem(&malformed(spec), None, Some("nodes"), "at most 100 nodes");
    nodes.truncate(98);
    assert!(malformed(spec_with(json!(nodes))).is_empty());

    let sources: Vec<Value> = (0..8)
        .map(|i| json!({"id": format!("s{i}"), "type": "source", "metric": "bytes"}))
        .collect();
    assert_problem(
        &malformed(spec_with(json!(sources))),
        None,
        Some("nodes"),
        "at most 8 sources",
    );
}

#[test]
fn nodes_off_the_path_are_allowed_and_checked() {
    let value = spec_with(json!([
        {"id": "unused", "type": "group", "input": "runs", "by": ["nothing"]},
        {"id": "chart", "type": "table", "input": "runs", "columns": ["y"]},
    ]));
    let spec = Spec::from_json(&value).unwrap();
    let graph = Graph::new(&spec).unwrap();
    let ids = |is: Vec<usize>| -> Vec<String> {
        is.into_iter().map(|i| spec.nodes[i].id.clone()).collect()
    };
    assert_eq!(
        ids(graph.upstream(graph.visualization())),
        ["runs", "chart"]
    );
    assert_problem(
        &problems(value),
        Some("unused"),
        Some("by[0]"),
        "neither an attribute",
    );
}

// Valid -------------------------------------------------------------------------------

#[test]
fn source_expressions_are_checked_with_their_positions() {
    let p = problems(spec_with(json!([
        {"id": "bad", "type": "source", "metric": "load_time +"},
        {"id": "unknown", "type": "source", "metric": "nothing * 2"},
        {"id": "unit", "type": "source", "metric": "load_time", "unit": "B"},
        {"id": "x", "type": "source", "metric": "series.fps", "x_unit": "B"},
        {"id": "scalar_x", "type": "source", "metric": "bytes", "x_unit": "s"},
    ])));
    let bad = p.iter().find(|p| p.node.as_deref() == Some("bad")).unwrap();
    assert_eq!(bad.setting.as_deref(), Some("metric"));
    assert!(bad.span.is_some_and(|s| s.start >= 10), "{bad:?}");
    let unknown = p
        .iter()
        .find(|p| p.node.as_deref() == Some("unknown"))
        .unwrap();
    assert_eq!(unknown.span.map(|s| (s.start, s.end)), Some((0, 7)));
    assert_problem(&p, Some("unit"), Some("metric"), "cannot be converted");
    assert_problem(
        &p,
        Some("x"),
        Some("x_unit"),
        "cannot be converted to \"B\"",
    );
    assert_problem(&p, Some("scalar_x"), Some("x_unit"), "gives a scalar");
}

#[test]
fn attributes_and_columns_must_exist() {
    let p = problems(spec_with(json!([
        {"id": "f", "type": "filter", "input": "runs", "conditions": [
            {"attribute": "sut.branch", "op": "=", "value": "main"},
            {"attribute": "sut.brunch", "op": "=", "value": "main"},
            {"attribute": "y", "op": ">", "value": 10},
            {"attribute": "attr.pixels", "op": "exists"}
        ]},
        {"id": "pts", "type": "filter", "input": "f", "on": "points", "conditions": [
            {"attribute": "y", "op": ">", "value": 10}
        ]},
        {"id": "d", "type": "derive", "input": "pts", "name": "pixels",
         "expression": "param.width * param.height"},
        {"id": "g", "type": "group", "input": "d", "by": ["attr.pixels", "cases"],
         "except": ["run"]},
        {"id": "s", "type": "sort", "input": "g", "target": "groups", "by": [{"key": "y"}]},
        {"id": "chart", "type": "line", "input": "s", "x": "attr.pixels", "y": "mean"}
    ])));
    assert_problem(
        &p,
        Some("f"),
        Some("conditions[1].attribute"),
        "no attribute \"sut.brunch\"",
    );
    assert_problem(
        &p,
        Some("f"),
        Some("conditions[2].attribute"),
        "set on to points",
    );
    assert_problem(
        &p,
        Some("f"),
        Some("conditions[3].attribute"),
        "no attribute \"attr.pixels\"",
    );
    assert_problem(&p, Some("g"), Some("by[1]"), "\"cases\" is neither");
    assert_problem(
        &p,
        Some("s"),
        Some("by[0].key"),
        "is a column; this takes an attribute",
    );
    assert_problem(
        &p,
        Some("chart"),
        Some("y"),
        "no column \"mean\"; the columns are y",
    );
    assert_eq!(p.len(), 6, "{p:#?}");
}

#[test]
fn group_levels_must_fit_the_visualization() {
    let p = problems(spec_with(json!([
        {"id": "g1", "type": "group", "input": "runs", "by": ["case"]},
        {"id": "g2", "type": "group", "input": "g1", "by": ["device.name"]},
        {"id": "g3", "type": "group", "input": "g2", "by": ["run.id"]},
        {"id": "chart", "type": "line", "input": "g3", "x": "run.started_at", "y": "y"}
    ])));
    assert_problem(
        &p,
        Some("chart"),
        Some("levels"),
        "3 group levels, but only 2",
    );
    let p = problems(spec_with(json!([
        {"id": "g1", "type": "group", "input": "runs", "by": ["case"]},
        {"id": "chart", "type": "line", "input": "g1", "x": "run.started_at", "y": "y",
         "levels": []}
    ])));
    assert_problem(
        &p,
        Some("chart"),
        Some("levels"),
        "1 group levels, but only 0",
    );
    let p = problems(spec_with(json!([
        {"id": "chart", "type": "bar", "input": "runs", "category": "device.name",
         "value": "y", "levels": ["color", "color"]}
    ])));
    assert_problem(&p, Some("chart"), Some("levels[1]"), "one group level");
}

#[test]
fn flatten_and_merging_nodes_remove_levels() {
    let v = validation(spec_with(json!([
        {"id": "g1", "type": "group", "input": "runs", "by": ["case"]},
        {"id": "g2", "type": "group", "input": "g1", "by": ["device.name"]},
        {"id": "g3", "type": "group", "input": "g2", "by": ["run.id"]},
        {"id": "one", "type": "flatten", "input": "g3"},
        {"id": "all", "type": "flatten", "input": "g3", "levels": "all"},
        {"id": "too_many", "type": "flatten", "input": "g1", "levels": 2},
        {"id": "none", "type": "flatten", "input": "runs"},
        {"id": "merged", "type": "combine", "input": "g3", "functions": ["mean"]},
        {"id": "chart", "type": "table", "input": "merged", "columns": ["mean"]}
    ])));
    let depth = |id: &str| v.shapes[id].levels.len();
    assert_eq!(
        ["one", "all", "too_many", "none", "merged"].map(depth),
        [2, 0, 0, 0, 2]
    );
    assert_problem(
        &v.problems,
        Some("too_many"),
        Some("levels"),
        "only 1 group levels, not 2",
    );
    assert_problem(&v.problems, Some("none"), None, "no group level to flatten");
}

#[test]
fn resample_needs_a_series() {
    let p = problems(spec_with(json!([
        {"id": "grid", "type": "resample", "input": "runs"}
    ])));
    assert_problem(&p, Some("grid"), None, "needs frames with an x column");
}

#[test]
fn combine_and_concatenate_check_what_they_merge() {
    let p = problems(spec_with(json!([
        {"id": "diff", "type": "combine", "input": "runs",
         "expressions": [{"name": "d", "expression": "a - b"}]},
        {"id": "other", "type": "source", "metric": "bytes"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "other"}},
        {"id": "cat", "type": "concatenate", "input": "pairs"},
        {"id": "red", "type": "reduce", "input": "pairs", "functions": ["mean"]},
        {"id": "fn", "type": "combine", "input": "pairs", "functions": ["mean"]},
        {"id": "dup", "type": "combine", "input": "pairs",
         "expressions": [{"name": "b", "expression": "a - b"}]},
        {"id": "chart", "type": "table", "input": "pairs", "columns": ["y"]}
    ])));
    assert_problem(&p, Some("diff"), None, "needs a match");
    assert_problem(&p, Some("cat"), None, "use combine");
    assert_problem(&p, Some("red"), None, "combine them first");
    assert_problem(&p, Some("fn"), None, "combine the inputs with expressions");
    assert_problem(
        &p,
        Some("dup"),
        Some("expressions[0].name"),
        "the column b exists",
    );
}

#[test]
fn reduce_needs_one_column_and_distinct_functions() {
    let p = problems(json!({"format": 1, "nodes": [
        {"id": "a", "type": "source", "metric": "load_time"},
        {"id": "b", "type": "source", "metric": "bytes"},
        {"id": "pairs", "type": "match", "inputs": {"a": "a", "b": "b"}},
        {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
        {"id": "which", "type": "reduce", "input": "both", "functions": ["mean"]},
        {"id": "named", "type": "reduce", "input": "both", "column": "a",
         "functions": ["mean", "mean", {"function": "percentile", "p": 95}]},
        {"id": "missing", "type": "reduce", "input": "both", "column": "c",
         "functions": ["mean"]},
        {"id": "chart", "type": "table", "input": "named", "columns": ["mean", "p95"]}
    ]}));
    assert_problem(
        &p,
        Some("which"),
        Some("column"),
        "the columns a, b: name one",
    );
    assert_problem(
        &p,
        Some("named"),
        Some("functions[1]"),
        "two functions give the column mean",
    );
    assert_problem(&p, Some("missing"), Some("column"), "no column \"c\"");
}

#[test]
fn axis_units_must_convert() {
    let p = problems(spec_with(json!([
        {"id": "chart", "type": "line", "input": "runs", "x": "run.started_at", "y": "y",
         "axes": {"y": {"unit": "s", "log": true}, "z": {"log": true}}}
    ])));
    assert_eq!(p.len(), 1, "{p:#?}");
    assert_problem(&p, Some("chart"), Some("axes.z"), "no axis z");
    let p = problems(spec_with(json!([
        {"id": "chart", "type": "line", "input": "runs", "x": "run.started_at", "y": "y",
         "axes": {"y": {"unit": "B"}}}
    ])));
    assert_problem(
        &p,
        Some("chart"),
        Some("axes.y.unit"),
        "cannot be converted to \"B\"",
    );
}

#[test]
fn differs_needs_a_matched_partner() {
    let p = problems(spec_with(json!([
        {"id": "chart", "type": "table", "input": "runs", "columns": ["y"],
         "differs": ["sut.id"]}
    ])));
    assert_problem(&p, Some("chart"), Some("differs[0]"), "the input has none");
}

#[test]
fn values_out_of_range_are_problems() {
    let p = problems(spec_with(json!([
        {"id": "latest", "type": "select_latest", "input": "runs", "runs": 0},
        {"id": "fps", "type": "source", "metric": "series.fps"},
        {"id": "grid", "type": "resample", "input": "fps", "points": 1, "from": 5, "to": 1},
        {"id": "stats", "type": "reduce", "input": "runs", "confidence": 1.5,
         "functions": [{"function": "percentile", "p": 120}, "percentile",
                       {"function": "mean", "p": 5}]},
        {"id": "both", "type": "combine", "input": "runs", "functions": ["stddev"],
         "expressions": []},
        {"id": "f", "type": "filter", "input": "runs", "conditions": [
            {"attribute": "run.id", "op": "exists", "value": 1},
            {"attribute": "run.id", "op": "in", "value": "a"},
            {"attribute": "run.started_at", "op": ">", "value": "yesterday"},
            {"attribute": "run.id", "op": "="}
        ]},
        {"id": "d", "type": "derive", "input": "runs", "name": "1x", "expression": " "}
    ])));
    assert_problem(&p, Some("latest"), Some("runs"), "at least one run");
    assert_problem(&p, Some("grid"), Some("points"), "2 to 10000 points, not 1");
    assert_problem(&p, Some("grid"), Some("to"), "greater than from");
    assert_problem(&p, Some("stats"), Some("confidence"), "between 0 and 1");
    assert_problem(&p, Some("stats"), Some("functions[0]"), "from 0 to 100");
    assert_problem(
        &p,
        Some("stats"),
        Some("functions[1]"),
        "percentile needs p",
    );
    assert_problem(&p, Some("stats"), Some("functions[2]"), "mean takes no p");
    assert_problem(&p, Some("both"), Some("expressions"), "not both");
    assert_problem(
        &p,
        Some("f"),
        Some("conditions[0].value"),
        "exists takes no value",
    );
    assert_problem(&p, Some("f"), Some("conditions[1].value"), "take a list");
    assert_problem(
        &p,
        Some("f"),
        Some("conditions[2].value"),
        "number or a time",
    );
    assert_problem(&p, Some("f"), Some("conditions[3].value"), "needs a value");
    assert_problem(&p, Some("d"), Some("name"), "\"1x\" is not a name");
    assert_problem(&p, Some("d"), Some("expression"), "empty");
}

#[test]
fn combine_functions_are_a_fixed_list() {
    let p = problems(spec_with(json!([
        {"id": "band", "type": "combine", "input": "runs", "functions": ["geomean"]}
    ])));
    assert_problem(
        &p,
        Some("band"),
        Some("functions[0]"),
        "geomean is not a function of combine",
    );
}

// Variables ---------------------------------------------------------------------------

#[test]
fn variables_must_exist_and_fit_their_settings() {
    let p = problems(json!({
      "format": 1,
      "variables": [
        {"name": "branch", "type": "text"},
        {"name": "limit", "type": "number", "default": "ten"},
        {"name": "branch", "type": "text"},
        {"name": "a b", "type": "text"},
        {"name": "dims", "type": "attribute", "list": true, "default": ["case"]}
      ],
      "nodes": [
        {"id": "runs", "type": "source", "metric": {"var": "branch"},
         "filter": {"sut_branch": {"var": "branch"}, "latest_runs": {"var": "nowhere"},
                    "started_after": "last week"}},
        {"id": "g", "type": "group", "input": "runs", "by": {"var": "dims"}},
        {"id": "chart", "type": "table", "input": "g", "columns": {"var": "branch"}}
      ]
    }));
    assert_problem(&p, None, Some("variables[1].default"), "expected number");
    assert_problem(&p, None, Some("variables[2].name"), "another variable");
    assert_problem(&p, None, Some("variables[3].name"), "not a variable name");
    assert_problem(
        &p,
        Some("runs"),
        Some("metric"),
        "is text, but the setting takes metric",
    );
    assert_problem(
        &p,
        Some("runs"),
        Some("filter.latest_runs"),
        "no variable \"nowhere\"",
    );
    assert_problem(&p, Some("runs"), Some("filter.started_after"), "not a time");
    assert_problem(
        &p,
        Some("chart"),
        Some("columns"),
        "takes a list of attribute",
    );
    assert!(
        !p.iter()
            .any(|p| p.setting.as_deref() == Some("filter.sut_branch"))
    );
    assert!(!p.iter().any(|p| p.node.as_deref() == Some("g")));
}

#[test]
fn a_variable_without_default_leaves_what_depends_on_it_unknown() {
    let v = validation(json!({
      "format": 1,
      "variables": [{"name": "metric", "type": "metric"},
                    {"name": "unit", "type": "text"},
                    {"name": "dims", "type": "attribute", "list": true}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": {"var": "metric"}},
        {"id": "ms", "type": "source", "metric": "load_time", "unit": {"var": "unit"}},
        {"id": "g", "type": "group", "input": "ms", "by": {"var": "dims"}},
        {"id": "chart", "type": "table", "input": "runs", "columns": ["nothing"]}
      ]
    }));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert!(!v.shapes.contains_key("runs"));
    assert!(!v.shapes.contains_key("chart"));
    assert_eq!(cols(&v.shapes["ms"]), [("y", None)]);
    assert_eq!(v.shapes["g"].levels.len(), 1);
}

#[test]
fn defaults_are_used_for_validation() {
    let v = validation(json!({
      "format": 1,
      "variables": [{"name": "metric", "type": "metric", "default": "series.fps"}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": {"var": "metric"}},
        {"id": "chart", "type": "line", "input": "runs", "x": "x", "y": "y"}
      ]
    }));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        cols(&v.shapes["runs"]),
        [("x", Some("s")), ("y", Some("fps"))]
    );
}

fn values(v: Value) -> BTreeMap<String, Value> {
    serde_json::from_value(v).unwrap()
}

#[test]
fn binding_replaces_variables_by_values_or_defaults() {
    let spec = Spec::from_json(&adr_example()).unwrap();
    let filter = |s: &Spec| match &s.nodes[0].kind {
        NodeKind::Source(src) => serde_json::to_value(&src.filter).unwrap(),
        _ => unreachable!(),
    };
    let bound = spec.bind(&values(json!({}))).unwrap();
    assert_eq!(
        filter(&bound),
        json!({"suite": "web-viewer", "sut_branch": "main"})
    );
    let bound = spec.bind(&values(json!({"branch": "dev"}))).unwrap();
    assert_eq!(
        filter(&bound),
        json!({"suite": "web-viewer", "sut_branch": "dev"})
    );

    let p = spec
        .bind(&values(json!({"branch": 5, "other": 1})))
        .unwrap_err();
    assert_problem(&p, None, Some("variables.branch"), "expected text");
    assert_problem(&p, None, Some("variables.other"), "no variable \"other\"");
}

#[test]
fn binding_needs_a_value_for_every_variable() {
    let value = json!({
      "format": 1,
      "variables": [{"name": "filter", "type": "run_filter"},
                    {"name": "points", "type": "number", "default": 1}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": "series.fps", "filter": {"var": "filter"}},
        {"id": "grid", "type": "resample", "input": "runs", "points": {"var": "points"}},
        {"id": "chart", "type": "line", "input": "grid", "x": "x", "y": "y"}
      ]
    });
    let spec = Spec::from_json(&value).unwrap();
    let p = spec.bind(&values(json!({}))).unwrap_err();
    assert_problem(
        &p,
        None,
        Some("variables.filter"),
        "no value and no default",
    );
    // The default is checked once bound: 1 is not a grid.
    let p = spec
        .bind(&values(json!({"filter": {"sut_branch": "main"}})))
        .unwrap_err();
    assert_eq!(p.len(), 1, "{p:#?}");
    assert_problem(&p, Some("grid"), Some("points"), "2 to 10000 points");
    let p = spec
        .bind(&values(json!({"filter": {"branch": "main"}, "points": 10})))
        .unwrap_err();
    assert_problem(&p, None, Some("variables.filter"), "no field \"branch\"");
    let bound = spec
        .bind(&values(
            json!({"filter": {"sut_branch": "main"}, "points": 10}),
        ))
        .unwrap();
    let graph = Graph::new(&bound).unwrap();
    let v = validate(&bound, &graph, &mut test_env(), &catalogue());
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        bound.to_json()["nodes"][1],
        json!({"id": "grid", "type": "resample", "input": "runs", "points": 10})
    );
}

#[test]
fn a_bound_variable_must_fit_the_setting() {
    // Validation reports the mismatch; binding refuses it too.
    let value = json!({
      "format": 1,
      "variables": [{"name": "branch", "type": "text", "default": "main"}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": {"var": "branch"}},
        {"id": "chart", "type": "table", "input": "runs", "columns": ["y"]}
      ]
    });
    let spec = Spec::from_json(&value).unwrap();
    let p = spec.bind(&values(json!({}))).unwrap_err();
    assert_problem(&p, Some("runs"), Some("metric"), "the setting takes metric");
}

#[test]
fn problems_serialize_with_their_position() {
    let p = problems(spec_with(json!([
        {"id": "bad", "type": "source", "metric": "unknown_metric"}
    ])));
    let bad = p.iter().find(|p| p.node.as_deref() == Some("bad")).unwrap();
    let json = serde_json::to_value(bad).unwrap();
    assert_eq!(json["node"], "bad");
    assert_eq!(json["setting"], "metric");
    assert_eq!(json["span"], json!({"start": 0, "end": 14}));
    assert_eq!(
        serde_json::to_value(Problem::spec("x")).unwrap(),
        json!({"message": "x"})
    );
}

#[test]
fn a_malformed_spec_reports_no_shapes() {
    let report = check(
        &json!({"format": 1, "nodes": [{"id": "a", "type": "pie"}]}),
        &mut test_env(),
        &catalogue(),
    );
    assert!(!report.well_formed && !report.valid);
    assert!(report.nodes.is_empty());
    assert_eq!(report.problems.len(), 1);
}

// Review of #39 ------------------------------------------------------------------------

#[test]
fn defaults_get_the_value_checks_of_their_settings() {
    let p = problems(json!({
      "format": 1,
      "variables": [{"name": "points", "type": "number", "default": 1},
                    {"name": "confidence", "type": "number", "default": 1.5},
                    {"name": "runs", "type": "number", "default": 2.5},
                    {"name": "latest", "type": "number", "default": 0}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": "series.fps",
         "filter": {"latest_runs": {"var": "latest"}}},
        {"id": "grid", "type": "resample", "input": "runs", "points": {"var": "points"}},
        {"id": "stats", "type": "reduce", "input": "grid", "column": "y",
         "functions": ["mean"], "confidence": {"var": "confidence"}},
        {"id": "latest", "type": "select_latest", "input": "stats", "runs": {"var": "runs"}},
        {"id": "chart", "type": "table", "input": "latest", "columns": ["mean"]}
      ]
    }));
    assert_problem(&p, Some("grid"), Some("points"), "2 to 10000 points, not 1");
    assert_problem(&p, Some("stats"), Some("confidence"), "between 0 and 1");
    assert_problem(
        &p,
        Some("latest"),
        Some("runs"),
        "default of variable \"runs\" does not fit",
    );
    assert_problem(&p, Some("runs"), Some("filter.latest_runs"), "at least 1");
}

#[test]
fn run_filter_values_are_checked_as_the_server_reads_them() {
    let p = problems(spec_with(json!([
        {"id": "s", "type": "source", "metric": "bytes",
         "filter": {"latest_runs": 1.5, "run_id": ["not-a-uuid"], "status": ["unknown"]}}
    ])));
    assert_problem(
        &p,
        Some("s"),
        Some("filter.latest_runs"),
        "whole number of at least 1",
    );
    assert_problem(&p, Some("s"), Some("filter.run_id"), "not a run id");
    assert_problem(&p, Some("s"), Some("filter.status"), "not one of running");
}

#[test]
fn flattening_a_match_needs_equal_columns() {
    let pairs = |b_metric: &str| {
        json!({"format": 1, "nodes": [
            {"id": "a", "type": "source", "metric": "load_time"},
            {"id": "b", "type": "source", "metric": b_metric},
            {"id": "pairs", "type": "match", "inputs": {"a": "a", "b": "b"}},
            {"id": "flat", "type": "flatten", "input": "pairs"},
            {"id": "stats", "type": "reduce", "input": "flat", "functions": ["mean"]},
            {"id": "chart", "type": "table", "input": "stats", "columns": ["mean"]}
        ]})
    };
    let v = validation(pairs("bytes"));
    assert_problem(
        &v.problems,
        Some("flat"),
        Some("levels"),
        "y in ms) and b (y in B)",
    );
    assert!(!v.shapes.contains_key("stats"));
    let v = validation(pairs("load_ms"));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(cols(&v.shapes["stats"])[0], ("mean", Some("ms")));
}

#[test]
fn an_empty_unit_declares_dimensionless() {
    let p = problems(spec_with(json!([
        {"id": "s", "type": "source", "metric": "load_time", "unit": ""}
    ])));
    assert_problem(&p, Some("s"), Some("metric"), "cannot be converted");
    let v = validation(spec_with(json!([
        {"id": "s", "type": "source", "metric": "score", "unit": ""}
    ])));
    assert_eq!(cols(&v.shapes["s"]), [("y", Some(""))]);
}

#[test]
fn combine_keeps_x_of_a_series_input_whatever_its_name() {
    let v = validation(json!({"format": 1, "nodes": [
        {"id": "limit", "type": "source", "metric": "bytes"},
        {"id": "series", "type": "source", "metric": "series.received"},
        {"id": "pairs", "type": "match", "inputs": {"a": "limit", "b": "series"}},
        {"id": "share", "type": "combine", "input": "pairs",
         "expressions": [{"name": "share", "expression": "b / a", "unit": ""}]},
        {"id": "chart", "type": "line", "input": "share", "x": "x", "y": "share"}
    ]}));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        cols(&v.shapes["share"]),
        [
            ("x", Some("s")),
            ("a", Some("B")),
            ("b", Some("B")),
            ("share", Some(""))
        ]
    );
    let p = problems(json!({"format": 1, "nodes": [
        {"id": "time", "type": "source", "metric": "series.fps"},
        {"id": "size", "type": "source", "metric": "series.by_size"},
        {"id": "pairs", "type": "match", "inputs": {"a": "time", "b": "size"}},
        {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
        {"id": "chart", "type": "table", "input": "both", "columns": ["a"]}
    ]}));
    assert_problem(
        &p,
        Some("both"),
        None,
        "x of a is in \"s\" and x of b in \"B\"",
    );
}

#[test]
fn x_cannot_name_a_match_input() {
    let p = malformed(spec_with(json!([
        {"id": "m", "type": "match", "inputs": {"a": "runs", "x": "runs"}}
    ])));
    assert_problem(&p, Some("m"), Some("inputs.x"), "x cannot name an input");
}

#[test]
fn bounds_and_comparisons_have_the_unit_of_their_value() {
    let v = |viz: Value| {
        problems(json!({"format": 1, "nodes": [
            {"id": "a", "type": "source", "metric": "load_time"},
            {"id": "b", "type": "source", "metric": "bytes"},
            {"id": "pairs", "type": "match", "inputs": {"a": "a", "b": "b"}},
            {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
            viz
        ]}))
    };
    let p = v(json!({"id": "chart", "type": "line", "input": "both",
                     "x": "run.started_at", "y": "a", "low": "b", "high": "a"}));
    assert_eq!(p.len(), 1, "{p:#?}");
    assert_problem(
        &p,
        Some("chart"),
        Some("low"),
        "b is in \"B\" and y (a) in \"ms\"",
    );
    let p = v(json!({"id": "chart", "type": "bar", "input": "both",
                     "category": "device.name", "value": "a", "high": "b"}));
    assert_problem(&p, Some("chart"), Some("high"), "cannot be converted");
    let p = v(json!({"id": "chart", "type": "value", "input": "both",
                     "value": "a", "compare": "b"}));
    assert_problem(&p, Some("chart"), Some("compare"), "cannot be converted");
}

#[test]
fn derive_expressions_read_numeric_attributes() {
    let p = problems(spec_with(json!([
        {"id": "a", "type": "derive", "input": "runs", "name": "a",
         "expression": "param.width * sut.id"},
        {"id": "b", "type": "derive", "input": "a", "name": "b",
         "expression": "attr.a / 2 + load_time"},
        {"id": "c", "type": "derive", "input": "b", "name": "c",
         "expression": "sut.branch + attr.nope + rate(param.w)"},
        {"id": "d", "type": "derive", "input": "c", "name": "d",
         "expression": "sut.branch + attr.nope"},
        {"id": "e", "type": "derive", "input": "d", "name": "e",
         "expression": "sample.repetition + 1[s]"},
        {"id": "f", "type": "derive", "input": "e", "name": "f",
         "expression": "sample.repetition * 2", "unit": "ms"},
        {"id": "g", "type": "derive", "input": "f", "name": "g",
         "expression": "attr.a * 2", "unit": "ms"},
        {"id": "h", "type": "derive", "input": "g", "name": "h",
         "expression": "abs(param.width, 1)"}
    ])));
    let at = |node: &str| -> Vec<&Problem> {
        p.iter()
            .filter(|x| x.node.as_deref() == Some(node))
            .collect()
    };
    assert!(at("a").is_empty(), "{p:#?}");
    assert_problem(
        &p,
        Some("b"),
        Some("expression"),
        "load_time is not an attribute",
    );
    assert_problem(
        &p,
        Some("c"),
        Some("expression"),
        "rate is not a function of attributes",
    );
    assert_problem(
        &p,
        Some("d"),
        Some("expression"),
        "sut.branch is not a number",
    );
    assert_problem(&p, Some("d"), Some("expression"), "no attribute attr.nope");
    assert_eq!(at("d")[0].span, Some(crate::dsl::Span::new(0, 10)));
    // Units are checked where they are known: the fixed attributes have none.
    assert_problem(&p, Some("e"), Some("expression"), "");
    assert_eq!(at("e").len(), 1, "{p:#?}");
    assert_problem(&p, Some("f"), Some("expression"), "cannot be converted");
    // A derived attribute's unit comes from the data: checked when it runs.
    assert!(at("g").is_empty(), "{p:#?}");
    // What does not depend on units is checked also with parameters.
    assert_problem(&p, Some("h"), Some("expression"), "abs takes 1 argument");
}

#[test]
fn a_derive_after_an_unknown_source_is_still_parsed() {
    let p = problems(json!({
        "format": 1,
        "variables": [{"name": "m", "type": "metric"}],
        "nodes": [
            {"id": "runs", "type": "source", "metric": {"var": "m"}},
            {"id": "d", "type": "derive", "input": "runs", "name": "w",
             "expression": "param.width +"},
            {"id": "chart", "type": "table", "input": "d", "columns": ["y"]}
        ]
    }));
    assert_problem(&p, Some("d"), Some("expression"), "");
    assert!(
        p.iter()
            .any(|x| x.node.as_deref() == Some("d") && x.span.is_some()),
        "{p:#?}"
    );
}

#[test]
fn a_parameter_with_an_empty_name_can_be_named() {
    let p = problems(spec_with(json!([
        {"id": "g", "type": "group", "input": "runs", "by": ["param."]},
        {"id": "f", "type": "flatten", "input": "g"},
        {"id": "d", "type": "derive", "input": "f", "name": "twice",
         "expression": "param.`` * 2"}
    ])));
    assert!(p.is_empty(), "{p:#?}");
}

#[test]
fn attributes_of_match_partners_are_checked_like_their_own() {
    let spec = |b: &str, nodes: Value| {
        let mut all = vec![
            json!({"id": "runs", "type": "source", "metric": "load_time"}),
            json!({"id": "ref", "type": "source", "metric": "load_time"}),
            json!({"id": "pairs", "type": "match", "inputs": {"a": "runs", b: "ref"},
                   "except": ["run", "sample"]}),
            json!({"id": "both", "type": "combine", "input": "pairs",
                   "expressions": [{"name": "ratio", "expression": format!("a / {b}"), "unit": ""}]}),
        ];
        all.extend(nodes.as_array().unwrap().iter().cloned());
        let last = all.last().unwrap()["id"].clone();
        all.push(json!({"id": "chart", "type": "table", "input": last, "columns": ["ratio"]}));
        json!({"format": 1, "nodes": all})
    };
    // A fixed attribute of a partner has no unit, as its own.
    let p = problems(spec(
        "b",
        json!([{"id": "d", "type": "derive", "input": "both", "name": "x",
                "expression": "b.sample.repetition + 1[s]"}]),
    ));
    assert_problem(&p, Some("d"), Some("expression"), "");
    // The parameter with an empty name of a partner can be named.
    let p = problems(spec(
        "b",
        json!([{"id": "g", "type": "group", "input": "both", "by": ["b.param."]}]),
    ));
    assert!(p.is_empty(), "{p:#?}");
    // A partner named series: its attributes, not a metric.
    let p = problems(spec(
        "series",
        json!([{"id": "d", "type": "derive", "input": "both", "name": "x",
                "expression": "series.param.width * 2 + series.sample.repetition"}]),
    ));
    assert!(p.is_empty(), "{p:#?}");
}

#[test]
fn the_unit_of_a_combine_expression_is_inferred() {
    let spec = |expressions: Value| {
        json!({"format": 1, "nodes": [
            {"id": "runs", "type": "source", "metric": "load_time"},
            {"id": "ref", "type": "source", "metric": "load_time"},
            {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
             "except": ["run", "sample"]},
            {"id": "both", "type": "combine", "input": "pairs", "expressions": expressions},
            {"id": "chart", "type": "table", "input": "both", "columns": ["a"]}
        ]})
    };
    let v = validation(spec(json!([
        {"name": "diff", "expression": "a - b"},
        {"name": "ratio", "expression": "a / b"},
        {"name": "pct", "expression": "a / b", "unit": "%"},
        {"name": "reps", "expression": "a / b.sample.repetition"},
        // A parameter has its unit in the data: known when the chart runs.
        {"name": "per_px", "expression": "a / param.width"},
    ])));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        cols(&v.shapes["both"]),
        [
            ("a", Some("ms")),
            ("b", Some("ms")),
            ("diff", Some("ms")),
            ("ratio", Some("ms/ms")),
            ("pct", Some("%")),
            ("reps", Some("ms")),
            ("per_px", None)
        ]
    );

    let p = problems(spec(json!([
        {"name": "e1", "expression": "a + c"},
        {"name": "e2", "expression": "a + run.id"},
        {"name": "e3", "expression": "b.mean"},
        {"name": "e4", "expression": "a + 1[s/B]"},
        {"name": "e5", "expression": "a + b.run.nope"},
        {"name": "e6", "expression": "rate(a)"},
    ])));
    let at = |i: usize| format!("expressions[{i}].expression");
    assert_problem(&p, Some("both"), Some(&at(0)), "unknown name c");
    assert!(
        p.iter()
            .any(|x| x.span == Some(crate::dsl::Span::new(4, 5)))
    );
    assert_problem(&p, Some("both"), Some(&at(1)), "run.id is not a number");
    assert_problem(&p, Some("both"), Some(&at(2)), "b has no column mean");
    assert_problem(&p, Some("both"), Some(&at(3)), "");
    assert_problem(&p, Some("both"), Some(&at(4)), "b has no column run.nope");
    assert_problem(
        &p,
        Some("both"),
        Some(&at(5)),
        "not a function of attributes",
    );
}

#[test]
fn the_input_named_a_comes_first_in_a_match() {
    let v = validation(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "bytes", "type": "source", "metric": "bytes"},
        {"id": "score", "type": "source", "metric": "score"},
        {"id": "pairs", "type": "match", "inputs": {"A": "bytes", "B": "score", "a": "runs"}},
        {"id": "both", "type": "combine", "input": "pairs",
         "expressions": [{"name": "e", "expression": "a + 1[s]"}]},
        {"id": "chart", "type": "table", "input": "both", "columns": ["a"]}
    ]}));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    let Level::Match { inputs } = &v.shapes["pairs"].levels[0] else {
        panic!("a match level")
    };
    assert_eq!(
        inputs.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        ["a", "A", "B"]
    );
    assert_eq!(
        v.shapes["pairs"].column("y").unwrap().unit.as_deref(),
        Some("ms")
    );
    assert_eq!(cols(&v.shapes["both"])[0], ("a", Some("ms")));
    assert_eq!(
        v.shapes["both"].column("e").unwrap().unit.as_deref(),
        Some("ms")
    );
}

#[test]
fn partners_get_the_attributes_derived_after_a_match() {
    let p = problems(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run", "sample"]},
        {"id": "twice", "type": "derive", "input": "pairs", "name": "twice",
         "expression": "sample.repetition * 2"},
        {"id": "both", "type": "combine", "input": "twice",
         "expressions": [{"name": "e", "expression": "a * b.attr.twice"}]},
        {"id": "chart", "type": "table", "input": "both", "columns": ["e"]}
    ]}));
    assert!(p.is_empty(), "{p:#?}");
}

#[test]
fn a_partner_name_used_again_replaces_the_earlier_partner() {
    // a carries b.attr.old from the first combine; the second combine
    // pairs it with a fresh b that has no attr.old, so b.attr.old is gone.
    let p = problems(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run", "sample"]},
        {"id": "old", "type": "derive", "input": "pairs", "name": "old",
         "expression": "sample.repetition * 2"},
        {"id": "first", "type": "combine", "input": "old",
         "expressions": [{"name": "e", "expression": "a / b"}]},
        {"id": "again", "type": "match", "inputs": {"a": "first", "b": "ref"},
         "except": ["run", "sample"]},
        {"id": "second", "type": "combine", "input": "again",
         "expressions": [{"name": "f", "expression": "a.e * b"}]},
        {"id": "use", "type": "derive", "input": "second", "name": "x",
         "expression": "b.attr.old"},
        {"id": "chart", "type": "table", "input": "use", "columns": ["f"]}
    ]}));
    assert_problem(&p, Some("use"), Some("expression"), "b.attr.old");
}

#[test]
fn a_combine_expression_cannot_grow_its_unit_without_bound() {
    let long = vec!["a"; 100].join(" * ");
    let spec = |unit: Value| {
        json!({"format": 1, "nodes": [
            {"id": "runs", "type": "source", "metric": "load_time"},
            {"id": "ref", "type": "source", "metric": "load_time"},
            {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"}},
            {"id": "both", "type": "combine", "input": "pairs",
             "expressions": [{"name": "e", "expression": long, "unit": unit}]},
            {"id": "chart", "type": "table", "input": "both", "columns": ["e"]}
        ]})
    };
    let p = problems(spec(Value::Null));
    assert_problem(
        &p,
        Some("both"),
        Some("expressions[0].unit"),
        "more than 256",
    );
    let p = problems(spec(json!("s^100")));
    assert!(p.is_empty(), "{p:#?}");
}

#[test]
fn combine_syntax_is_checked_without_its_input() {
    let p = problems(json!({
      "format": 1,
      "variables": [{"name": "metric", "type": "metric"}],
      "nodes": [
        {"id": "runs", "type": "source", "metric": {"var": "metric"}},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"}},
        {"id": "both", "type": "combine", "input": "pairs",
         "expressions": [{"name": "e", "expression": "a +"}]},
        {"id": "chart", "type": "table", "input": "both", "columns": ["e"]}
      ]
    }));
    assert_problem(&p, Some("both"), Some("expressions[0].expression"), "");
}

#[test]
fn a_reduce_inside_a_match_updates_the_columns_of_its_inputs() {
    let v = validation(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"]},
        {"id": "by_source", "type": "group", "input": "pairs", "by": ["source"]},
        {"id": "stats", "type": "reduce", "input": "by_source", "functions": ["mean"]},
        {"id": "diff", "type": "combine", "input": "stats",
         "expressions": [{"name": "d", "expression": "a.mean - b.mean"}]},
        {"id": "chart", "type": "table", "input": "diff", "columns": ["d"]}
    ]}));
    assert!(v.problems.is_empty(), "{:#?}", v.problems);
    assert_eq!(
        v.shapes["diff"].column("d").unwrap().unit.as_deref(),
        Some("ms")
    );
}

#[test]
fn a_reduce_inside_a_match_reads_the_column_of_each_input() {
    // b is reduced before the match, so its only column is mean in B; a
    // reduce without a column reads a's y and b's mean.
    let p = problems(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "bytes"},
        {"id": "ref_mean", "type": "reduce", "input": "ref", "functions": ["mean"]},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref_mean"},
         "except": ["run", "sample"]},
        {"id": "by_source", "type": "group", "input": "pairs", "by": ["source"]},
        {"id": "stats", "type": "reduce", "input": "by_source", "functions": ["mean"]},
        {"id": "sum", "type": "combine", "input": "stats",
         "expressions": [{"name": "d", "expression": "a.mean + b.mean"}]},
        {"id": "chart", "type": "table", "input": "sum", "columns": ["d"]}
    ]}));
    assert_problem(
        &p,
        Some("sum"),
        Some("expressions[0].expression"),
        "compatible units",
    );
}

#[test]
fn a_reduce_below_a_deeper_match_updates_its_inputs() {
    let p = problems(json!({"format": 1, "nodes": [
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"]},
        {"id": "by_source", "type": "group", "input": "pairs", "by": ["source"]},
        {"id": "by_run", "type": "group", "input": "by_source", "by": ["run.id"]},
        {"id": "stats", "type": "reduce", "input": "by_run", "functions": ["mean"]},
        {"id": "flat", "type": "flatten", "input": "stats"},
        {"id": "diff", "type": "combine", "input": "flat",
         "expressions": [{"name": "d", "expression": "a.mean - b.mean"}]},
        {"id": "chart", "type": "table", "input": "diff", "columns": ["d"]}
    ]}));
    assert!(p.is_empty(), "{p:#?}");
}
