//! Tests of the nodes of #28 and of the evaluation of a graph.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use serde_json::{Value, json};

use super::*;
use crate::chart::frame::{Attr, AttrValue, Attributes, Column, Frame, Group, Tree};
use crate::chart::spec::{
    DeriveNode, FilterNode, FlattenNode, GroupNode, SelectLatestNode, SortNode,
};
use crate::units::tests::catalogue;

// Builders -------------------------------------------------------------------------------

fn text(s: &str) -> AttrValue {
    AttrValue::Text(s.into())
}

fn num(v: impl Into<serde_json::Number>) -> AttrValue {
    AttrValue::Number(v.into())
}

fn time(day: u32) -> AttrValue {
    AttrValue::Time(Utc.with_ymd_and_hms(2026, 11, day, 0, 0, 0).unwrap())
}

/// Attributes from `(name, value)`; `param.*` in px.
fn attrs(list: &[(&str, AttrValue)]) -> Attributes {
    list.iter()
        .map(|(n, v)| {
            let unit = if n.starts_with("param.") { "px" } else { "" };
            ((*n).to_owned(), Attr::with_unit(v.clone(), unit))
        })
        .collect()
}

/// A frame with the column `y` in ms.
fn frame(list: &[(&str, AttrValue)], ys: &[Option<f64>]) -> Frame {
    Frame::new(attrs(list), vec![Column::new("y", "ms")], vec![ys.to_vec()])
}

/// The frame of a sample of run `run`, started on day `day`.
fn sample(run: &str, day: u32, ys: &[f64]) -> Frame {
    frame(
        &[("run.id", text(run)), ("run.started_at", time(day))],
        &ys.iter().map(|y| Some(*y)).collect::<Vec<_>>(),
    )
}

fn node<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn ys(f: &Frame) -> Vec<Option<f64>> {
    f.column_values("y").unwrap().to_vec()
}

fn attr<'a>(f: &'a Frame, name: &str) -> Option<&'a AttrValue> {
    f.attrs.get(name).map(|a| &a.value)
}

fn groups(tree: &Tree) -> &[Group] {
    match tree {
        Tree::Groups(g) => g,
        Tree::Frames(_) => panic!("expected groups"),
    }
}

fn frames(tree: &Tree) -> &[Frame] {
    match tree {
        Tree::Frames(f) => f,
        Tree::Groups(_) => panic!("expected frames"),
    }
}

fn run_ids(frames: &[Frame]) -> Vec<String> {
    frames
        .iter()
        .map(|f| match attr(f, "run.id") {
            Some(AttrValue::Text(s)) => s.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

// filter ---------------------------------------------------------------------------------

fn filtered(spec: Value, tree: Tree) -> Result<Tree> {
    structure::filter("f", &node::<FilterNode>(spec), tree)
}

#[test]
fn filter_keeps_the_frames_that_meet_all_conditions() {
    let fs = vec![
        frame(
            &[("param.w", num(10)), ("device.name", text("a"))],
            &[Some(1.0)],
        ),
        frame(
            &[("param.w", num(20)), ("device.name", text("b"))],
            &[Some(2.0)],
        ),
        frame(&[("device.name", text("a"))], &[Some(3.0)]),
        frame(&[("param.w", text("10"))], &[Some(4.0)]),
    ];
    let run = |conds: Value| {
        let out = filtered(json!({"conditions": conds}), Tree::Frames(fs.clone())).unwrap();
        frames(&out)
            .iter()
            .map(|f| ys(f)[0].unwrap())
            .collect::<Vec<_>>()
    };
    // Values keep their JSON type: 10 is not "10", but 10 is 10.0.
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "=", "value": 10}])),
        [1.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "=", "value": 10.0}])),
        [1.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "=", "value": "10"}])),
        [4.0]
    );
    // A missing attribute meets no condition, != and not in included.
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "!=", "value": 10}])),
        [2.0, 4.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "not in", "value": [20]}])),
        [1.0, 4.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "in", "value": [20, "10"]}])),
        [2.0, 4.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": ">", "value": 10}])),
        [2.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "<=", "value": 10}])),
        [1.0]
    );
    assert_eq!(
        run(json!([{"attribute": "param.w", "op": "exists"}])),
        [1.0, 2.0, 4.0]
    );
    assert_eq!(
        run(json!([
            {"attribute": "device.name", "op": "=", "value": "a"},
            {"attribute": "param.w", "op": "exists"}
        ])),
        [1.0]
    );
}

#[test]
fn filter_compares_times_and_reads_lists_as_contains() {
    let tags = |t: &[&str]| AttrValue::List(t.iter().map(|s| text(s)).collect());
    let fs = vec![
        frame(
            &[
                ("run.started_at", time(1)),
                ("run.tags", tags(&["nightly", "gpu"])),
            ],
            &[Some(1.0)],
        ),
        frame(
            &[("run.started_at", time(5)), ("run.tags", tags(&["manual"]))],
            &[Some(2.0)],
        ),
    ];
    let run = |cond: Value| {
        let out = filtered(json!({"conditions": [cond]}), Tree::Frames(fs.clone())).unwrap();
        frames(&out)
            .iter()
            .map(|f| ys(f)[0].unwrap())
            .collect::<Vec<_>>()
    };
    let at = |op: &str| json!({"attribute": "run.started_at", "op": op, "value": "2026-11-03T00:00:00Z"});
    assert_eq!(run(at(">=")), [2.0]);
    assert_eq!(run(at("<")), [1.0]);
    assert_eq!(
        run(
            json!({"attribute": "run.started_at", "op": "=", "value": "2026-11-05T01:00:00+01:00"})
        ),
        [2.0]
    );
    assert_eq!(
        run(json!({"attribute": "run.tags", "op": "=", "value": "gpu"})),
        [1.0]
    );
    assert_eq!(
        run(json!({"attribute": "run.tags", "op": "!=", "value": "gpu"})),
        [2.0]
    );
    assert_eq!(
        run(json!({"attribute": "run.tags", "op": "in", "value": ["manual", "x"]})),
        [2.0]
    );
}

#[test]
fn filter_on_points_reads_columns_and_removes_what_becomes_empty() {
    let tree = Tree::Groups(vec![
        Group {
            key: vec![("device.name".into(), Some(text("a")))],
            tree: Tree::Frames(vec![frame(&[], &[Some(1.0), None, Some(5.0)])]),
        },
        Group {
            key: vec![("device.name".into(), Some(text("b")))],
            tree: Tree::Frames(vec![frame(&[], &[Some(0.5)])]),
        },
    ]);
    let out = filtered(
        json!({"on": "points", "conditions": [{"attribute": "y", "op": ">=", "value": 1}]}),
        tree.clone(),
    )
    .unwrap();
    let g = groups(&out);
    assert_eq!(g.len(), 1, "the group of b became empty");
    assert_eq!(ys(&frames(&g[0].tree)[0]), [Some(1.0), Some(5.0)]);

    // No value never meets a condition, not even != or exists.
    let out = filtered(
        json!({"on": "points", "conditions": [{"attribute": "y", "op": "exists"}]}),
        tree,
    )
    .unwrap();
    assert_eq!(
        ys(&frames(&groups(&out)[0].tree)[0]),
        [Some(1.0), Some(5.0)]
    );

    // A frame that no longer meets the conditions leaves its group empty.
    let out = filtered(
        json!({"conditions": [{"attribute": "device.name", "op": "=", "value": "x"}]}),
        Tree::Groups(vec![Group {
            key: vec![],
            tree: Tree::Frames(vec![frame(&[("device.name", text("a"))], &[Some(1.0)])]),
        }]),
    )
    .unwrap();
    assert!(groups(&out).is_empty());
}

// group ----------------------------------------------------------------------------------

#[test]
fn group_orders_subgroups_by_key_and_keeps_the_order_of_frames() {
    let fs = vec![
        frame(
            &[("device.name", text("b")), ("param.w", num(2))],
            &[Some(1.0)],
        ),
        frame(
            &[("device.name", text("a")), ("param.w", num(2))],
            &[Some(2.0)],
        ),
        frame(&[("param.w", num(1))], &[Some(3.0)]),
        frame(
            &[("device.name", text("a")), ("param.w", num(1))],
            &[Some(4.0)],
        ),
        frame(
            &[("device.name", text("b")), ("param.w", num(2))],
            &[Some(5.0)],
        ),
    ];
    let out = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["device.name"]})),
        Tree::Frames(fs.clone()),
    )
    .unwrap();
    let g = groups(&out);
    let keys: Vec<_> = g.iter().map(|g| g.key.clone()).collect();
    assert_eq!(
        keys,
        [
            vec![("device.name".to_owned(), Some(text("a")))],
            vec![("device.name".to_owned(), Some(text("b")))],
            // Frames without a value form a subgroup of their own, last.
            vec![("device.name".to_owned(), None)],
        ]
    );
    let ys_of = |g: &Group| {
        frames(&g.tree)
            .iter()
            .map(|f| ys(f)[0].unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(ys_of(&g[0]), [2.0, 4.0]);
    assert_eq!(ys_of(&g[1]), [1.0, 5.0]);
    assert_eq!(ys_of(&g[2]), [3.0]);

    // A family stands for its attributes; except takes some out again.
    let out = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["device", "case"], "except": ["device.details"]})),
        Tree::Frames(fs.clone()),
    )
    .unwrap();
    let g = groups(&out);
    assert_eq!(g.len(), 4);
    assert_eq!(
        g[0].key,
        vec![
            ("device.name".to_owned(), Some(text("a"))),
            ("param.w".to_owned(), Some(num(1))),
        ]
    );

    // A second group adds a level below every group.
    let out = structure::group("g2", &node::<GroupNode>(json!({"by": ["param.w"]})), out).unwrap();
    assert_eq!(out.depth(), 2);
    out.check().unwrap();
}

// flatten --------------------------------------------------------------------------------

#[test]
fn flatten_removes_levels_from_the_inside() {
    let fs: Vec<Frame> = (0..4)
        .map(|i| {
            frame(
                &[
                    ("device.name", text(["a", "b"][i % 2])),
                    ("param.w", num(i / 2)),
                ],
                &[Some(i as f64)],
            )
        })
        .collect();
    let by = |key: &str| node::<GroupNode>(json!({"by": [key]}));
    let tree = structure::group("g", &by("device.name"), Tree::Frames(fs)).unwrap();
    let tree = structure::group("g", &by("param.w"), tree).unwrap();
    assert_eq!(tree.depth(), 2);

    let one = structure::flatten(&node::<FlattenNode>(json!({})), tree.clone());
    assert_eq!(one.depth(), 1);
    assert_eq!(groups(&one)[0].key[0].0, "device.name");
    let ys: Vec<f64> = one.frames().iter().map(|f| ys(f)[0].unwrap()).collect();
    assert_eq!(ys, [0.0, 2.0, 1.0, 3.0]);

    let all = structure::flatten(&node::<FlattenNode>(json!({"levels": "all"})), tree.clone());
    assert_eq!(all.depth(), 0);
    assert_eq!(frames(&all).len(), 4);
    let more = structure::flatten(&node::<FlattenNode>(json!({"levels": 5})), tree);
    assert_eq!(more, all);
}

// sort -----------------------------------------------------------------------------------

#[test]
fn sort_orders_frames_groups_and_points() {
    let fs = vec![
        frame(
            &[("param.w", num(2)), ("device.name", text("a"))],
            &[Some(1.0)],
        ),
        frame(&[("device.name", text("b"))], &[Some(2.0)]),
        frame(
            &[("param.w", num(3)), ("device.name", text("a"))],
            &[Some(3.0)],
        ),
        frame(
            &[("param.w", num(2)), ("device.name", text("b"))],
            &[Some(4.0)],
        ),
    ];
    let sorted = |by: Value, tree: Tree| {
        structure::sort("s", &node::<SortNode>(json!({"by": by})), tree).unwrap()
    };
    let ys_of = |t: &Tree| {
        t.frames()
            .iter()
            .map(|f| ys(f)[0].unwrap())
            .collect::<Vec<_>>()
    };
    // Missing values last, also descending; stable.
    let out = sorted(
        json!([{"key": "param.w", "desc": true}]),
        Tree::Frames(fs.clone()),
    );
    assert_eq!(ys_of(&out), [3.0, 1.0, 4.0, 2.0]);
    let out = sorted(
        json!([{"key": "param.w"}, {"key": "device.name", "desc": true}]),
        Tree::Frames(fs.clone()),
    );
    assert_eq!(ys_of(&out), [4.0, 1.0, 3.0, 2.0]);

    // Groups by their key, or by the value their frames share.
    let tree = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["device.name"]})),
        Tree::Frames(fs),
    )
    .unwrap();
    let out = structure::sort(
        "s",
        &node::<SortNode>(
            json!({"target": "groups", "by": [{"key": "device.name", "desc": true}]}),
        ),
        tree,
    )
    .unwrap();
    assert_eq!(groups(&out)[0].key[0].1, Some(text("b")));
    let err = structure::sort(
        "s",
        &node::<SortNode>(json!({"target": "groups", "by": [{"key": "device.name"}]})),
        Tree::Frames(vec![]),
    )
    .unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("target"));

    // Points by a column: no value last; attributes move with their points.
    let mut f = frame(&[], &[Some(3.0), None, Some(1.0)]);
    f.point_attrs = Some(vec![
        attrs(&[("run.id", text("r3"))]),
        attrs(&[("run.id", text("rn"))]),
        attrs(&[("run.id", text("r1"))]),
    ]);
    let out = sorted_points(f);
    let f = &frames(&out)[0];
    assert_eq!(ys(f), [Some(1.0), Some(3.0), None]);
    assert_eq!(f.point_attr(0, "run.id").unwrap().value, text("r1"));
    assert_eq!(f.point_attr(2, "run.id").unwrap().value, text("rn"));
}

fn sorted_points(f: Frame) -> Tree {
    structure::sort(
        "s",
        &node::<SortNode>(json!({"target": "points", "by": [{"key": "y"}]})),
        Tree::Frames(vec![f]),
    )
    .unwrap()
}

// select_latest --------------------------------------------------------------------------

#[test]
fn select_latest_keeps_the_frames_of_the_latest_runs_per_group() {
    let fs = vec![
        sample("r1", 1, &[1.0]),
        sample("r3", 3, &[3.0]),
        sample("r2", 2, &[2.0]),
        sample("r3", 3, &[3.5]),
        // Same start: the greater id is the later run, as in the run list.
        sample("r0", 3, &[0.0]),
        frame(&[], &[Some(9.0)]),
    ];
    let latest = |v: Value, tree: Tree| {
        structure::select_latest("l", &node::<SelectLatestNode>(v), tree).unwrap()
    };
    let out = latest(json!({}), Tree::Frames(fs.clone()));
    assert_eq!(run_ids(frames(&out)), ["r3", "r3"]);
    let out = latest(json!({"runs": 3}), Tree::Frames(fs.clone()));
    assert_eq!(run_ids(frames(&out)), ["r3", "r2", "r3", "r0"]);

    // Per innermost group.
    let tree = Tree::Groups(vec![
        Group {
            key: vec![],
            tree: Tree::Frames(fs[..3].to_vec()),
        },
        Group {
            key: vec![],
            tree: Tree::Frames(vec![fs[0].clone(), fs[5].clone()]),
        },
        Group {
            key: vec![],
            tree: Tree::Frames(vec![fs[5].clone()]),
        },
    ]);
    let out = latest(json!({"runs": 1}), tree);
    let g = groups(&out);
    assert_eq!(g.len(), 2, "a group without runs is removed");
    assert_eq!(run_ids(frames(&g[0].tree)), ["r3"]);
    assert_eq!(run_ids(frames(&g[1].tree)), ["r1"]);
}

#[test]
fn select_latest_on_points_reads_the_runs_of_the_points() {
    let concat = structure::concatenate(
        "c",
        Tree::Frames(vec![
            sample("r1", 1, &[1.0, 1.5]),
            sample("r2", 2, &[2.0]),
            sample("r3", 3, &[3.0]),
        ]),
    )
    .unwrap();
    let out = structure::select_latest(
        "l",
        &node::<SelectLatestNode>(json!({"runs": 2, "target": "points"})),
        concat,
    )
    .unwrap();
    let f = &frames(&out)[0];
    assert_eq!(ys(f), [Some(2.0), Some(3.0)]);
    assert_eq!(f.point_attr(0, "run.id").unwrap().value, text("r2"));
}

// concatenate ----------------------------------------------------------------------------

#[test]
fn concatenate_merges_each_innermost_group_and_points_keep_their_attributes() {
    let mut a = sample("r1", 1, &[1.0, 2.0]);
    a.attrs.insert("device.name".into(), Attr::new(text("d")));
    let mut b = sample("r2", 2, &[3.0]);
    b.attrs.insert("device.name".into(), Attr::new(text("d")));
    let c = sample("r3", 3, &[4.0]);
    let tree = Tree::Groups(vec![
        Group {
            key: vec![("device.name".into(), Some(text("d")))],
            tree: Tree::Frames(vec![a, b]),
        },
        Group {
            key: vec![("device.name".into(), None)],
            tree: Tree::Frames(vec![c]),
        },
    ]);
    let out = structure::concatenate("c", tree).unwrap();
    // The level of the merged groups disappears.
    let fs = frames(&out);
    assert_eq!(fs.len(), 2);
    let f = &fs[0];
    assert_eq!(ys(f), [Some(1.0), Some(2.0), Some(3.0)]);
    // The frame keeps what the merged frames share ...
    assert_eq!(attr(f, "device.name"), Some(&text("d")));
    assert_eq!(attr(f, "run.id"), None);
    // ... and every point the attributes of its frame.
    assert_eq!(f.point_attr(1, "run.id").unwrap().value, text("r1"));
    assert_eq!(f.point_attr(2, "run.id").unwrap().value, text("r2"));
    assert_eq!(f.point_attr(2, "run.started_at").unwrap().value, time(2));
    assert_eq!(f.point_attr(2, "device.name").unwrap().value, text("d"));
    assert_eq!(fs[1].point_attrs, None, "one frame: nothing differs");
    out.check().unwrap();

    // A flat list becomes one frame; nothing stays nothing.
    let out = structure::concatenate(
        "c",
        Tree::Frames(vec![sample("r1", 1, &[1.0]), sample("r2", 2, &[2.0])]),
    )
    .unwrap();
    assert_eq!(frames(&out).len(), 1);
    assert_eq!(
        frames(&structure::concatenate("c", Tree::default()).unwrap()).len(),
        0
    );

    // Concatenating again keeps the attributes of every point.
    let twice = structure::concatenate(
        "c",
        Tree::Frames(vec![frames(&out)[0].clone(), sample("r3", 3, &[3.0])]),
    )
    .unwrap();
    let f = &frames(&twice)[0];
    let runs: Vec<_> = (0..3)
        .map(|i| f.point_attr(i, "run.id").unwrap().value.clone())
        .collect();
    assert_eq!(runs, [text("r1"), text("r2"), text("r3")]);
}

#[test]
fn concatenate_needs_the_same_columns() {
    let a = frame(&[], &[Some(1.0)]);
    let mut b = frame(&[], &[Some(1.0)]);
    b.columns[0].unit = "s".into();
    let err = structure::concatenate("c", Tree::Frames(vec![a, b])).unwrap_err();
    assert_eq!(err.node.as_deref(), Some("c"));
    assert!(
        err.message.contains("y in ms") && err.message.contains("y in s"),
        "{err}"
    );
}

// derive ---------------------------------------------------------------------------------

fn derived(spec: Value, tree: Tree) -> Result<Tree> {
    derive::derive("d", &node::<DeriveNode>(spec), tree, &catalogue())
}

#[test]
fn derive_computes_an_attribute_with_units() {
    let mut mm = frame(&[("param.w", num(3)), ("param.h", num(4))], &[Some(1.0)]);
    mm.attrs.get_mut("param.h").unwrap().unit = "mm".into();
    let fs = vec![
        frame(&[("param.w", num(3)), ("param.h", num(4))], &[Some(1.0)]),
        // Without an attribute, or with one that is not a number: no value.
        frame(&[("param.w", num(3))], &[Some(2.0)]),
        frame(&[("param.w", num(3)), ("param.h", text("4"))], &[Some(3.0)]),
        // Units that do not fit the declared unit: no value either.
        mm,
    ];
    let out = derived(
        json!({"name": "pixels", "expression": "param.w * param.h", "unit": "px^2"}),
        Tree::Frames(fs.clone()),
    )
    .unwrap();
    let got: Vec<Option<&Attr>> = frames(&out)
        .iter()
        .map(|f| f.attrs.get("attr.pixels"))
        .collect();
    assert_eq!(
        got[0],
        Some(&Attr::with_unit(AttrValue::number(12.0).unwrap(), "px^2"))
    );
    assert_eq!(&got[1..], [None, None, None]);

    // Without a declared unit the attribute has the unit of the expression,
    // here per frame; no value (a division by zero) gives no attribute.
    let out = derived(
        json!({"name": "ratio", "expression": "param.w / (param.h - 4)"}),
        Tree::Frames(fs),
    )
    .unwrap();
    assert!(
        frames(&out)
            .iter()
            .all(|f| !f.attrs.contains_key("attr.ratio"))
    );

    let out = derived(
        json!({"name": "half", "expression": "sample.repetition / 2 + abs(-1)"}),
        Tree::Frames(vec![frame(&[("sample.repetition", num(3))], &[Some(1.0)])]),
    )
    .unwrap();
    assert_eq!(
        frames(&out)[0].attrs["attr.half"],
        Attr::new(AttrValue::number(2.5).unwrap())
    );
}

#[test]
fn derive_gives_points_their_own_value() {
    let mut a = frame(&[("param.w", num(2))], &[Some(1.0)]);
    a.attrs.insert("device.name".into(), Attr::new(text("d")));
    let b = frame(&[("param.w", num(5))], &[Some(2.0)]);
    let out = structure::concatenate("c", Tree::Frames(vec![a, b])).unwrap();
    let out = derived(json!({"name": "w2", "expression": "param.w * 2"}), out).unwrap();
    let f = &frames(&out)[0];
    assert!(!f.attrs.contains_key("attr.w2"), "the frame has no param.w");
    let w2 = |i| {
        f.point_attr(i, "attr.w2")
            .map(|a| a.value.as_f64().unwrap())
    };
    assert_eq!((w2(0), w2(1)), (Some(4.0), Some(10.0)));
}

#[test]
fn derive_rejects_a_broken_expression() {
    let err = derived(
        json!({"name": "x", "expression": "load_time * 2"}),
        Tree::default(),
    )
    .unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("expression"));
    assert!(err.span.is_some());
}

// run ------------------------------------------------------------------------------------

fn bound_spec(nodes: Value) -> (Spec, Graph) {
    let spec = Spec::from_json(&json!({"format": 1, "nodes": nodes})).unwrap();
    let spec = spec.bind(&BTreeMap::new()).unwrap();
    let graph = Graph::new(&spec).unwrap();
    (spec, graph)
}

#[test]
fn a_graph_runs_from_its_sources_to_the_visualization() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "per_run", "type": "group", "input": "runs", "by": ["run.id"]},
        {"id": "samples", "type": "concatenate", "input": "per_run"},
        {"id": "latest", "type": "select_latest", "input": "samples", "runs": 2},
        {"id": "chart", "type": "table", "input": "latest", "columns": ["y"]}
    ]));
    let source = Tree::Frames(vec![
        sample("r1", 1, &[1.0]),
        sample("r2", 2, &[2.0]),
        sample("r1", 1, &[1.5]),
        sample("r3", 3, &[3.0]),
    ]);
    let sources = || BTreeMap::from([("runs".to_owned(), source.clone())]);
    let out = run(&spec, &graph, sources(), None, &catalogue()).unwrap();
    assert_eq!(run_ids(frames(&out)), ["r2", "r3"]);

    // A preview stops at a node.
    let out = run(&spec, &graph, sources(), Some("per_run"), &catalogue()).unwrap();
    assert_eq!(groups(&out).len(), 3);
    assert_eq!(ys(&frames(&groups(&out)[0].tree)[1]), [Some(1.5)]);

    let err = run(&spec, &graph, sources(), Some("nope"), &catalogue()).unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("until"));
    let err = run(&spec, &graph, BTreeMap::new(), None, &catalogue()).unwrap_err();
    assert_eq!(err.node.as_deref(), Some("runs"));
}

#[test]
fn a_source_with_two_readers_feeds_both() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "a", "type": "filter", "input": "runs",
         "conditions": [{"attribute": "run.id", "op": "=", "value": "r1"}]},
        {"id": "b", "type": "filter", "input": "runs",
         "conditions": [{"attribute": "run.id", "op": "=", "value": "r2"}]},
        {"id": "m", "type": "match", "inputs": {"a": "a", "b": "b"}, "except": ["run"]},
        {"id": "chart", "type": "table", "input": "m", "columns": ["y"]}
    ]));
    let source = Tree::Frames(vec![sample("r1", 1, &[1.0]), sample("r2", 2, &[2.0])]);
    // Both filters read the source; the match is not evaluated yet (#29).
    let err = run(
        &spec,
        &graph,
        BTreeMap::from([("runs".to_owned(), source.clone())]),
        None,
        &catalogue(),
    )
    .unwrap_err();
    assert_eq!(err.node.as_deref(), Some("m"));
    let out = run(
        &spec,
        &graph,
        BTreeMap::from([("runs".to_owned(), source)]),
        Some("b"),
        &catalogue(),
    )
    .unwrap();
    assert_eq!(run_ids(frames(&out)), ["r2"]);
}
