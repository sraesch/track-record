//! Tests of the nodes (#28, #29) and of the evaluation of a graph.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use serde_json::{Value, json};

use super::*;
use crate::chart::frame::{Attr, AttrValue, Attributes, Column, Frame, Group, Tree};
use crate::chart::spec::{
    CombineNode, DeriveNode, FilterNode, FlattenNode, GroupNode, MatchNode, ReduceNode,
    ResampleNode, SelectLatestNode, SortNode,
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
            key: vec![("device.name".into(), Some(Attr::new(text("a"))))],
            tree: Tree::Frames(vec![frame(&[], &[Some(1.0), None, Some(5.0)])]),
        },
        Group {
            key: vec![("device.name".into(), Some(Attr::new(text("b"))))],
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
            vec![("device.name".to_owned(), Some(Attr::new(text("a"))))],
            vec![("device.name".to_owned(), Some(Attr::new(text("b"))))],
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
            ("device.name".to_owned(), Some(Attr::new(text("a")))),
            ("param.w".to_owned(), Some(Attr::with_unit(num(1), "px"))),
        ]
    );

    // A second group adds a level below every group.
    let out = structure::group("g2", &node::<GroupNode>(json!({"by": ["param.w"]})), out).unwrap();
    assert_eq!(out.depth(), 2);
    out.check().unwrap();
}

#[test]
fn group_keeps_cases_with_the_same_value_in_different_units_apart() {
    let mut kbit = frame(&[("param.bw", num(10))], &[Some(2.0)]);
    kbit.attrs.get_mut("param.bw").unwrap().unit = "kbit/s".into();
    let fs = vec![frame(&[("param.bw", num(10))], &[Some(1.0)]), kbit];
    let out = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["case"]})),
        Tree::Frames(fs),
    )
    .unwrap();
    let g = groups(&out);
    assert_eq!(g.len(), 2, "{g:#?}");
    let unit = |g: &Group| g.key[0].1.as_ref().unwrap().unit.clone();
    assert_eq!(
        (unit(&g[0]), unit(&g[1])),
        ("kbit/s".to_owned(), "px".to_owned())
    );
}

#[test]
fn a_parameter_with_an_empty_name_is_part_of_the_case() {
    let fs = vec![
        frame(&[("param.", num(1))], &[Some(1.0)]),
        frame(&[("param.", num(2))], &[Some(2.0)]),
    ];
    let out = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["case"]})),
        Tree::Frames(fs),
    )
    .unwrap();
    let keys: Vec<_> = groups(&out)
        .iter()
        .map(|g| {
            let (name, attr) = &g.key[0];
            (
                g.key.len(),
                name.clone(),
                attr.as_ref().map(|a| a.value.clone()),
            )
        })
        .collect();
    assert_eq!(
        keys,
        vec![
            (1, "param.".to_owned(), Some(num(1))),
            (1, "param.".to_owned(), Some(num(2))),
        ]
    );
    // An expression names it with an empty quoted name.
    let out = derived(
        json!({"name": "twice", "expression": "param.`` * 2"}),
        Tree::Frames(vec![frame(&[("param.", num(3))], &[Some(1.0)])]),
    )
    .unwrap();
    assert_eq!(
        frames(&out)[0].attrs["attr.twice"].value.as_f64(),
        Some(6.0)
    );
}

#[test]
fn group_keys_name_what_the_spec_names_and_what_families_find() {
    // An attribute the key names is in every key, also when no frame has it.
    let fs = vec![frame(&[("device.name", text("a"))], &[Some(1.0)])];
    let out = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["sut.tag", "device.name"]})),
        Tree::Frames(fs),
    )
    .unwrap();
    assert_eq!(
        groups(&out)[0].key,
        vec![
            ("sut.tag".to_owned(), None),
            ("device.name".to_owned(), Some(Attr::new(text("a")))),
        ]
    );

    // A family stands for the attributes the frames have; a frame without
    // one of them sorts after those with it, as a missing value does.
    let fs = vec![
        frame(&[("param.a", num(1))], &[Some(1.0)]),
        frame(&[("param.b", num(1))], &[Some(2.0)]),
        frame(&[("param.a", num(1)), ("param.b", num(1))], &[Some(3.0)]),
        frame(&[("param.a", num(0))], &[Some(4.0)]),
    ];
    let out = structure::group(
        "g",
        &node::<GroupNode>(json!({"by": ["case"]})),
        Tree::Frames(fs),
    )
    .unwrap();
    let firsts: Vec<f64> = groups(&out)
        .iter()
        .map(|g| ys(&frames(&g.tree)[0])[0].unwrap())
        .collect();
    assert_eq!(firsts, [4.0, 3.0, 1.0, 2.0]);
    let names: Vec<Vec<&str>> = groups(&out)
        .iter()
        .map(|g| g.key.iter().map(|(n, _)| n.as_str()).collect())
        .collect();
    assert_eq!(names[1], ["param.a", "param.b"]);
    assert_eq!(names[3], ["param.b"]);
}

#[test]
fn an_empty_result_passes_through_nodes_that_need_levels() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "d", "type": "group", "input": "runs", "by": ["device.name"]},
        {"id": "r", "type": "group", "input": "d", "by": ["run.id"]},
        {"id": "f", "type": "flatten", "input": "r"},
        {"id": "s", "type": "sort", "input": "f", "target": "groups",
         "by": [{"key": "device.name"}]},
        {"id": "chart", "type": "table", "input": "s", "columns": ["y"]}
    ]));
    let sources = BTreeMap::from([("runs".to_owned(), Tree::default())]);
    let out = run(&spec, &graph, sources, None, &catalogue(), usize::MAX).unwrap();
    assert!(out.frames().is_empty());
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
    assert_eq!(groups(&out)[0].key[0].1, Some(Attr::new(text("b"))));
    let err = structure::sort(
        "s",
        &node::<SortNode>(json!({"target": "groups", "by": [{"key": "device.name"}]})),
        Tree::Frames(vec![frame(&[], &[Some(1.0)])]),
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
            key: vec![("device.name".into(), Some(Attr::new(text("d"))))],
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
fn a_derived_unit_reads_back_in_the_next_derive() {
    let mut f = frame(&[("param.area", num(9))], &[Some(1.0)]);
    f.attrs.get_mut("param.area").unwrap().unit = "px^2".into();
    let out = derived(
        json!({"name": "side", "expression": "sqrt(param.area)"}),
        Tree::Frames(vec![f]),
    )
    .unwrap();
    let out = derived(
        json!({"name": "width", "expression": "attr.side + 1[px]"}),
        out,
    )
    .unwrap();
    let f = &frames(&out)[0];
    assert_eq!(f.attrs["attr.width"].value.as_f64(), Some(4.0));
    // `+` gives the unit of its first operand, which converts to px.
    let unit = &f.attrs["attr.width"].unit;
    assert_eq!(unit, "sqrt(px^2)");
    assert_eq!(catalogue().conversion_factor(unit, "px"), Some(1.0));
}

#[test]
fn a_custom_unit_with_spaces_reads_back_in_the_next_derive() {
    let mut f = frame(&[("param.n", num(3))], &[Some(1.0)]);
    f.attrs.get_mut("param.n").unwrap().unit = "render passes".into();
    let tree = Tree::Frames(vec![f]);
    let out = derived(
        json!({"name": "sq", "expression": "param.n * param.n"}),
        tree,
    )
    .unwrap();
    let out = derived(json!({"name": "root", "expression": "sqrt(attr.sq)"}), out).unwrap();
    let out = derived(
        json!({"name": "sum", "expression": "attr.root + param.n"}),
        out,
    )
    .unwrap();
    let f = &frames(&out)[0];
    assert_eq!(f.attrs["attr.sq"].unit, "render passes*render passes");
    assert_eq!(
        f.attrs["attr.root"].unit,
        "sqrt(render passes*render passes)"
    );
    assert_eq!(f.attrs["attr.sum"].value.as_f64(), Some(6.0));
}

#[test]
fn chained_derives_cannot_grow_units_without_bound() {
    let mut tree = Tree::Frames(vec![frame(&[("param.w", num(1))], &[Some(1.0)])]);
    tree = derived(json!({"name": "a0", "expression": "param.w"}), tree).unwrap();
    let mut failed = None;
    for i in 1..20 {
        let spec = json!({"name": format!("a{i}"), "expression": format!("attr.a{} * attr.a{}", i - 1, i - 1)});
        match derived(spec, tree.clone()) {
            Ok(t) => tree = t,
            Err(e) => {
                failed = Some((i, e));
                break;
            }
        }
    }
    let (i, err) = failed.expect("the unit grows past the limit");
    assert!(i < 10, "stopped at {i}");
    assert_eq!(err.setting.as_deref(), Some("unit"));
    assert!(err.message.contains("declare a unit"), "{err}");
    // A declared unit keeps it short.
    let unit = format!("px^{}", 1u64 << i);
    let spec = json!({"name": "x", "expression": format!("attr.a{} * attr.a{}", i - 1, i - 1), "unit": unit});
    let out = derived(spec, tree).unwrap();
    assert_eq!(frames(&out)[0].attrs["attr.x"].unit, unit);
}

#[test]
fn a_long_unit_that_a_derive_passes_on_is_kept() {
    let long = "x".repeat(derive::MAX_UNIT_LEN + 1);
    let mut f = frame(&[], &[Some(1.0)]);
    f.attrs
        .insert("param.w".into(), Attr::with_unit(num(2), long.clone()));
    let tree = Tree::Frames(vec![f]);
    for spec in [
        json!({"name": "a", "expression": "param.w * 3"}),
        json!({"name": "a", "expression": "param.w", "unit": long}),
    ] {
        let out = derived(spec, tree.clone()).unwrap();
        assert_eq!(frames(&out)[0].attrs["attr.a"].unit, long);
    }
    // Made by the expression, it is too long.
    let err = derived(
        json!({"name": "a", "expression": "param.w * param.w"}),
        tree,
    )
    .unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("unit"));
}

#[test]
fn the_unit_limit_counts_characters() {
    let micro = "µ".repeat(70);
    let mut f = frame(&[], &[Some(1.0)]);
    f.attrs
        .insert("param.w".into(), Attr::with_unit(num(2), micro.clone()));
    // 141 characters, 281 bytes.
    let out = derived(
        json!({"name": "a", "expression": "param.w * param.w"}),
        Tree::Frames(vec![f]),
    )
    .unwrap();
    assert_eq!(
        frames(&out)[0].attrs["attr.a"].unit,
        format!("{micro}*{micro}")
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
    let out = run(&spec, &graph, sources(), None, &catalogue(), usize::MAX).unwrap();
    assert_eq!(run_ids(frames(&out)), ["r2", "r3"]);

    // A preview stops at a node.
    let out = run(
        &spec,
        &graph,
        sources(),
        Some("per_run"),
        &catalogue(),
        usize::MAX,
    )
    .unwrap();
    assert_eq!(groups(&out).len(), 3);
    assert_eq!(ys(&frames(&groups(&out)[0].tree)[1]), [Some(1.5)]);

    let err = run(
        &spec,
        &graph,
        sources(),
        Some("nope"),
        &catalogue(),
        usize::MAX,
    )
    .unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("until"));
    let err = run(
        &spec,
        &graph,
        BTreeMap::new(),
        None,
        &catalogue(),
        usize::MAX,
    )
    .unwrap_err();
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
    // Both filters read the source: the match pairs r1 with r2.
    let out = run(
        &spec,
        &graph,
        BTreeMap::from([("runs".to_owned(), source.clone())]),
        None,
        &catalogue(),
        usize::MAX,
    )
    .unwrap();
    let pair = frames(&groups(&out)[0].tree);
    assert_eq!(run_ids(pair), ["r1", "r2"]);
    assert_eq!(pair[1].input.as_deref(), Some("b"));
    let out = run(
        &spec,
        &graph,
        BTreeMap::from([("runs".to_owned(), source)]),
        Some("b"),
        &catalogue(),
        usize::MAX,
    )
    .unwrap();
    assert_eq!(run_ids(frames(&out)), ["r2"]);
}

// Computing nodes (#29) ------------------------------------------------------------------

/// The values of a column.
fn col(f: &Frame, name: &str) -> Vec<Option<f64>> {
    f.column_values(name)
        .unwrap_or_else(|| panic!("no column {name} in {:?}", f.columns))
        .to_vec()
}

/// Names and units of the columns.
fn columns(f: &Frame) -> Vec<(&str, &str)> {
    f.columns
        .iter()
        .map(|c| (c.name.as_str(), c.unit.as_str()))
        .collect()
}

fn close(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() <= 1e-9 * b.abs().max(1.0))
}

/// A series in fps over x in `x_unit` (time).
fn series_frame(list: &[(&str, AttrValue)], x_unit: &str, xs: &[f64], ys: &[Option<f64>]) -> Frame {
    Frame::new(
        attrs(list),
        vec![Column::x(x_unit, "time"), Column::new("y", "fps")],
        vec![xs.iter().map(|x| Some(*x)).collect(), ys.to_vec()],
    )
}

fn reduced(spec: Value, tree: Tree) -> Result<Tree> {
    compute::reduce("r", &node::<ReduceNode>(spec), tree, &catalogue())
}

#[test]
fn reduce_computes_every_function_of_a_frame() {
    let ys = [
        Some(2.0),
        Some(4.0),
        None,
        Some(4.0),
        Some(4.0),
        Some(5.0),
        Some(5.0),
        Some(7.0),
        Some(9.0),
    ];
    let out = reduced(
        json!({"functions": ["mean", "median", "min", "max", "sum", "stddev",
                             {"function": "percentile", "p": 95}, "ci_low", "ci_high",
                             "geomean", "first", "last", "count"],
               "confidence": 0.9}),
        Tree::Frames(vec![frame(&[("run.id", text("r1"))], &ys)]),
    )
    .unwrap();
    let [r] = frames(&out) else {
        panic!("one frame")
    };
    assert_eq!(
        columns(r),
        [
            ("mean", "ms"),
            ("median", "ms"),
            ("min", "ms"),
            ("max", "ms"),
            ("sum", "ms"),
            ("stddev", "ms"),
            ("p95", "ms"),
            ("ci_low", "ms"),
            ("ci_high", "ms"),
            ("geomean", "ms"),
            ("first", "ms"),
            ("last", "ms"),
            ("count", ""),
            ("n", ""),
            ("n_no_value", "")
        ]
    );
    let s = crate::stats::summarize(&ys, &[95.0], 0.9);
    let one = |name: &str| {
        let v = col(r, name);
        assert_eq!(v.len(), 1);
        v[0]
    };
    assert_eq!(one("mean"), Some(5.0));
    assert_eq!(one("median"), s.median);
    assert_eq!((one("min"), one("max")), (Some(2.0), Some(9.0)));
    assert_eq!(one("sum"), Some(40.0));
    assert_eq!(one("stddev"), s.stddev);
    assert_eq!(one("p95"), s.percentiles[0].1);
    assert_eq!(
        (one("ci_low"), one("ci_high")),
        (s.ci.map(|c| c.0), s.ci.map(|c| c.1))
    );
    let ln: f64 = ys.iter().flatten().map(|y| y.ln()).sum::<f64>() / 8.0;
    assert!(close(one("geomean"), ln.exp()));
    assert_eq!((one("first"), one("last")), (Some(2.0), Some(9.0)));
    assert_eq!(one("count"), Some(8.0));
    assert_eq!((one("n"), one("n_no_value")), (Some(8.0), Some(1.0)));
    // The frame's attributes stay with the frame: there is one.
    assert_eq!(attr(r, "run.id"), Some(&text("r1")));
    assert!(r.point_attrs.is_none());
}

#[test]
fn reduce_gives_a_point_per_frame_with_its_own_attributes() {
    let fs = vec![
        frame(
            &[
                ("run.id", text("r1")),
                ("run.started_at", time(1)),
                ("param.w", num(10)),
            ],
            &[Some(1.0), Some(-3.0)],
        ),
        frame(
            &[
                ("run.id", text("r2")),
                ("run.started_at", time(2)),
                ("param.w", num(10)),
            ],
            &[Some(4.0)],
        ),
        frame(
            &[
                ("run.id", text("r1")),
                ("run.started_at", time(1)),
                ("param.w", num(20)),
            ],
            &[None],
        ),
    ];
    let grouped =
        structure::group("g", &node(json!({"by": ["param.w"]})), Tree::Frames(fs)).unwrap();
    let out = reduced(json!({"functions": ["mean", "stddev", "geomean"]}), grouped).unwrap();
    let [w10, w20] = frames(&out) else {
        panic!("one frame per group")
    };
    // The group level is gone; the frames keep what they share.
    assert_eq!(attr(w10, "param.w"), Some(&num(10)));
    assert_eq!(attr(w10, "run.id"), None);
    assert_eq!(col(w10, "mean"), [Some(-1.0), Some(4.0)]);
    // The stddev of one point and the geomean of a negative are no value.
    assert_eq!(col(w10, "stddev")[1], None);
    assert_eq!(col(w10, "geomean"), [None, Some(4.0)]);
    assert_eq!(col(w10, "n"), [Some(2.0), Some(1.0)]);
    let points = w10.point_attrs.as_ref().unwrap();
    assert_eq!(points[0]["run.id"].value, text("r1"));
    assert_eq!(w10.point_attr(1, "run.started_at").unwrap().value, time(2));
    assert_eq!(w10.point_attr(1, "param.w").unwrap().value, num(10));
    // A frame of no values: counted, nothing computed.
    assert_eq!(col(w20, "mean"), [None]);
    assert_eq!(
        (col(w20, "n"), col(w20, "n_no_value")),
        (vec![Some(0.0)], vec![Some(1.0)])
    );
    // A single frame shares all its attributes: no point attributes.
    assert_eq!(attr(w20, "run.id"), Some(&text("r1")));
    assert!(w20.point_attrs.is_none());

    // An empty list gives no frame.
    let out = reduced(json!({"functions": ["mean"]}), Tree::Frames(vec![])).unwrap();
    assert!(frames(&out).is_empty());
}

#[test]
fn reduce_converts_units_and_needs_one_column() {
    let mut s = frame(&[("run.id", text("r2"))], &[Some(0.002)]);
    s.columns[0].unit = "s".into();
    let out = reduced(
        json!({"functions": ["max"]}),
        Tree::Frames(vec![
            frame(&[("run.id", text("r1"))], &[Some(1.0)]),
            s.clone(),
        ]),
    )
    .unwrap();
    assert_eq!(col(&frames(&out)[0], "max"), [Some(1.0), Some(2.0)]);
    s.columns[0].unit = "B".into();
    let err = reduced(
        json!({"functions": ["max"]}),
        Tree::Frames(vec![frame(&[], &[Some(1.0)]), s]),
    )
    .unwrap_err();
    assert!(err.message.contains("cannot be converted"), "{err}");

    let mut two = frame(&[], &[Some(1.0)]);
    two.columns.push(Column::new("z", "ms"));
    two.values.push(vec![Some(2.0)]);
    let err = reduced(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![two.clone()]),
    )
    .unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("column"));
    assert!(err.message.contains("y, z"), "{err}");
    let err = reduced(
        json!({"functions": ["mean"], "column": "q"}),
        Tree::Frames(vec![two.clone()]),
    )
    .unwrap_err();
    assert!(err.message.contains("there is no column \"q\""), "{err}");
    let out = reduced(
        json!({"functions": ["mean"], "column": "z"}),
        Tree::Frames(vec![two]),
    )
    .unwrap();
    assert_eq!(col(&frames(&out)[0], "mean"), [Some(2.0)]);
}

fn resampled(spec: Value, tree: Tree) -> Result<Tree> {
    compute::resample(
        "g",
        &node::<ResampleNode>(spec),
        tree,
        &catalogue(),
        usize::MAX,
    )
}

#[test]
fn resample_puts_the_frames_of_a_group_on_one_grid() {
    let fs = vec![
        series_frame(
            &[("run.id", text("r1"))],
            "s",
            &[0.0, 2.0, 4.0],
            &[Some(0.0), Some(20.0), Some(40.0)],
        ),
        series_frame(
            &[("run.id", text("r2"))],
            "s",
            &[1.0, 3.0],
            &[Some(10.0), None],
        ),
    ];
    let out = resampled(json!({"points": 5}), Tree::Frames(fs.clone())).unwrap();
    let [a, b] = frames(&out) else { panic!() };
    assert_eq!(
        col(a, "x"),
        [Some(0.0), Some(1.0), Some(2.0), Some(3.0), Some(4.0)]
    );
    assert_eq!(col(b, "x"), col(a, "x"));
    assert_eq!(a.columns[0], Column::x("s", "time"));
    assert_eq!(
        col(a, "y"),
        [Some(0.0), Some(10.0), Some(20.0), Some(30.0), Some(40.0)]
    );
    // Before a series, next to no value and after it there is no value.
    assert_eq!(col(b, "y"), [None, Some(10.0), None, None, None]);
    assert_eq!(attr(b, "run.id"), Some(&text("r2")));

    let out = resampled(
        json!({"points": 5, "interp": "step"}),
        Tree::Frames(fs.clone()),
    )
    .unwrap();
    let [a, b] = frames(&out) else { panic!() };
    assert_eq!(
        col(a, "y"),
        [Some(0.0), Some(0.0), Some(20.0), Some(20.0), Some(40.0)]
    );
    assert_eq!(col(b, "y"), [None, Some(10.0), Some(10.0), None, None]);

    // An explicit grid.
    let out = resampled(json!({"points": 3, "from": 0, "to": 1}), Tree::Frames(fs)).unwrap();
    assert_eq!(
        col(&frames(&out)[0], "y"),
        [Some(0.0), Some(5.0), Some(10.0)]
    );
    assert_eq!(col(&frames(&out)[1], "y"), [None, None, Some(10.0)]);
}

#[test]
fn resample_grids_each_innermost_group_and_needs_x() {
    let group = |key: i32, fs: Vec<Frame>| Group {
        key: vec![("param.w".into(), Some(Attr::with_unit(num(key), "px")))],
        tree: Tree::Frames(fs),
    };
    let mut ordered = series_frame(&[], "s", &[10.0, 20.0], &[Some(1.0), Some(2.0)]);
    ordered.point_attrs = Some(vec![Attributes::new(), Attributes::new()]);
    let tree = Tree::Groups(vec![
        group(
            1,
            vec![series_frame(&[], "s", &[0.0, 1.0], &[Some(0.0), Some(1.0)])],
        ),
        // Points not in the order of x are put in order; a frame without
        // points gives no values on the grid.
        group(
            2,
            vec![
                ordered,
                series_frame(&[], "s", &[20.0, 10.0], &[Some(4.0), Some(3.0)]),
                series_frame(&[], "s", &[], &[]),
            ],
        ),
        // Without any points there is no grid.
        group(3, vec![series_frame(&[], "s", &[], &[])]),
    ]);
    let out = resampled(json!({"points": 3}), tree).unwrap();
    let g = groups(&out);
    assert_eq!(
        col(&frames(&g[0].tree)[0], "x"),
        [Some(0.0), Some(0.5), Some(1.0)]
    );
    let second = frames(&g[1].tree);
    assert_eq!(col(&second[0], "x"), [Some(10.0), Some(15.0), Some(20.0)]);
    assert_eq!(col(&second[1], "y"), [Some(3.0), Some(3.5), Some(4.0)]);
    assert_eq!(col(&second[2], "y"), [None, None, None]);
    assert!(second[0].point_attrs.is_none(), "the points are new");
    assert!(frames(&g[2].tree)[0].is_empty());

    let err = resampled(json!({}), Tree::Frames(vec![frame(&[], &[Some(1.0)])])).unwrap_err();
    assert_eq!(err.node.as_deref(), Some("g"));
    assert!(err.message.contains("x column"), "{err}");
}

fn matched(spec: Value, inputs: Vec<(&str, Tree)>) -> Result<Tree> {
    compute::match_frames(
        "m",
        &node::<MatchNode>(spec),
        inputs.into_iter().map(|(n, t)| (n.to_owned(), t)).collect(),
        usize::MAX,
    )
}

/// A frame of run `run` with the parameter `w` and the source `source`.
fn case_frame(source: &str, run: &str, w: AttrValue, ys: &[Option<f64>]) -> Frame {
    frame(
        &[
            ("source", text(source)),
            ("run.id", text(run)),
            ("param.w", w),
        ],
        ys,
    )
}

#[test]
fn match_pairs_frames_with_the_same_key() {
    let a = Tree::Frames(vec![
        case_frame("runs", "r1", num(10), &[Some(1.0)]),
        case_frame("runs", "r1", num(20), &[Some(2.0)]),
        case_frame("runs", "r1", num(30), &[Some(3.0)]),
        case_frame("runs", "r2", num(10), &[Some(4.0)]),
    ]);
    // 10 is 10.0; the source is never part of the key.
    let b = Tree::Frames(vec![
        case_frame("ref", "ref", AttrValue::number(10.0).unwrap(), &[Some(5.0)]),
        case_frame("ref", "ref", num(20), &[Some(6.0)]),
    ]);
    let out = matched(
        json!({"except": ["run"]}),
        vec![("a", a.clone()), ("b", b.clone())],
    )
    .unwrap();
    let g = groups(&out);
    assert_eq!(g.len(), 3, "the frame of w = 30 has no partner");
    assert_eq!(
        g[0].key,
        [("param.w".to_owned(), Some(Attr::with_unit(num(10), "px")))]
    );
    let pair = frames(&g[0].tree);
    assert_eq!(run_ids(pair), ["r1", "ref"]);
    assert_eq!(
        pair.iter().map(|f| f.input.as_deref()).collect::<Vec<_>>(),
        [Some("a"), Some("b")]
    );
    // A partner of several frames of a is in each group.
    assert_eq!(run_ids(frames(&g[2].tree)), ["r2", "ref"]);

    let out = matched(
        json!({"except": ["run"], "unmatched": "keep"}),
        vec![("a", a.clone()), ("b", b.clone())],
    )
    .unwrap();
    assert_eq!(run_ids(frames(&groups(&out)[2].tree)), ["r1"]);

    // With only, other attributes do not count; without except, the runs
    // differ.
    let out = matched(
        json!({"only": ["param.w"]}),
        vec![("a", a.clone()), ("b", b.clone())],
    )
    .unwrap();
    assert_eq!(groups(&out).len(), 3);
    let out = matched(json!({}), vec![("a", a.clone()), ("b", b.clone())]).unwrap();
    assert!(groups(&out).is_empty());

    // The same value in another unit is another key.
    let mut mm = case_frame("ref", "ref", num(10), &[Some(5.0)]);
    mm.attrs.get_mut("param.w").unwrap().unit = "mm".into();
    let out = matched(
        json!({"except": ["run"]}),
        vec![("a", a.clone()), ("b", Tree::Frames(vec![mm]))],
    )
    .unwrap();
    assert!(groups(&out).is_empty());

    // Two partners with one key.
    let twice = Tree::Frames(vec![
        case_frame("ref", "x", num(10), &[Some(5.0)]),
        case_frame("ref", "y", num(10), &[Some(6.0)]),
    ]);
    let err = matched(
        json!({"except": ["run"]}),
        vec![("a", a.clone()), ("b", twice)],
    )
    .unwrap_err();
    assert!(
        err.message
            .contains("input b has two frames with the key param.w = 10 px"),
        "{err}"
    );

    // The groups of a stay; each frame becomes a group below them.
    let grouped = structure::group("g", &node(json!({"by": ["run.id"]})), a).unwrap();
    let out = matched(json!({"except": ["run"]}), vec![("a", grouped), ("b", b)]).unwrap();
    out.check().unwrap();
    assert_eq!(out.depth(), 2);
    assert_eq!(groups(&groups(&out)[0].tree).len(), 2);
}

fn combined(spec: Value, tree: Tree) -> Result<Tree> {
    compute::combine("c", &node::<CombineNode>(spec), tree, None, &catalogue())
}

#[test]
fn combine_with_functions_works_point_by_point() {
    let fs = vec![
        series_frame(
            &[("run.id", text("r1")), ("device.name", text("d"))],
            "s",
            &[0.0, 1.0, 2.0],
            &[Some(1.0), Some(2.0), Some(3.0)],
        ),
        series_frame(
            &[("run.id", text("r2")), ("device.name", text("d"))],
            "s",
            &[0.0, 1.0, 2.0],
            &[Some(3.0), None, Some(5.0)],
        ),
        // x in ms lines up with x in s.
        series_frame(
            &[("run.id", text("r3")), ("device.name", text("d"))],
            "ms",
            &[0.0, 1000.0, 2000.0],
            &[Some(5.0), Some(6.0), None],
        ),
    ];
    let out = combined(
        json!({"functions": ["mean", "median", {"function": "percentile", "p": 90}, "sum", "count"]}),
        Tree::Frames(fs.clone()),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(
        columns(c),
        [
            ("x", "s"),
            ("mean", "fps"),
            ("median", "fps"),
            ("p90", "fps"),
            ("sum", "fps"),
            ("count", ""),
            ("n", ""),
            ("n_no_value", "")
        ]
    );
    assert_eq!(col(c, "x"), [Some(0.0), Some(1.0), Some(2.0)]);
    assert_eq!(col(c, "mean"), [Some(3.0), Some(4.0), Some(4.0)]);
    assert_eq!(col(c, "median"), [Some(3.0), Some(4.0), Some(4.0)]);
    assert!(close(col(c, "p90")[0], 4.6));
    assert_eq!(col(c, "sum"), [Some(9.0), Some(8.0), Some(8.0)]);
    // No value is left out and not counted.
    assert_eq!(col(c, "count"), [Some(3.0), Some(2.0), Some(2.0)]);
    // Every aggregate states how many values had none (FR-12a).
    assert_eq!(col(c, "n"), [Some(3.0), Some(2.0), Some(2.0)]);
    assert_eq!(col(c, "n_no_value"), [Some(0.0), Some(1.0), Some(1.0)]);
    assert_eq!(attr(c, "device.name"), Some(&text("d")));
    assert_eq!(attr(c, "run.id"), None);

    // A frame with one point and no x applies to every point.
    let mut with_scalar = fs.clone();
    with_scalar.push(frame(&[], &[Some(2.0)]));
    with_scalar
        .iter_mut()
        .for_each(|f| f.columns.last_mut().unwrap().unit = "ms".into());
    let out = combined(
        json!({"functions": ["min", "max"]}),
        Tree::Frames(with_scalar),
    )
    .unwrap();
    assert_eq!(
        col(&frames(&out)[0], "min"),
        [Some(1.0), Some(2.0), Some(2.0)]
    );

    // Frames on different x need a resample first.
    let mut apart = fs.clone();
    apart[1] = series_frame(
        &[],
        "s",
        &[0.0, 1.5, 2.0],
        &[Some(1.0), Some(2.0), Some(3.0)],
    );
    let err = combined(json!({"functions": ["mean"]}), Tree::Frames(apart)).unwrap_err();
    assert!(
        err.message.contains("x values differ") && err.message.contains("resample"),
        "{err}"
    );
    let err = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![
            fs[0].clone(),
            frame(&[], &[Some(1.0), Some(2.0), Some(3.0)]),
        ]),
    )
    .unwrap_err();
    assert!(err.message.contains("one has x"), "{err}");
}

#[test]
fn combine_with_functions_keeps_the_point_attributes_the_frames_share() {
    // Two devices reduced per run: the points are runs.
    let fs = vec![
        frame(
            &[
                ("device.name", text("d1")),
                ("run.id", text("r1")),
                ("run.started_at", time(1)),
            ],
            &[Some(1.0)],
        ),
        frame(
            &[
                ("device.name", text("d1")),
                ("run.id", text("r2")),
                ("run.started_at", time(2)),
            ],
            &[Some(3.0)],
        ),
        frame(
            &[
                ("device.name", text("d2")),
                ("run.id", text("r1")),
                ("run.started_at", time(1)),
            ],
            &[Some(5.0)],
        ),
        frame(
            &[
                ("device.name", text("d2")),
                ("run.id", text("r2")),
                ("run.started_at", time(2)),
            ],
            &[Some(7.0)],
        ),
    ];
    let per_device =
        structure::group("g", &node(json!({"by": ["device.name"]})), Tree::Frames(fs)).unwrap();
    let stats = reduced(json!({"functions": ["mean"]}), per_device).unwrap();
    let out = combined(json!({"column": "mean", "functions": ["mean"]}), stats).unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(col(c, "mean"), [Some(3.0), Some(5.0)]);
    assert_eq!(c.point_attr(1, "run.started_at").unwrap().value, time(2));
    assert!(c.point_attr(0, "device.name").is_none());
}

/// Frames of a and b matched on everything but the run.
fn pairs(a: Vec<Frame>, b: Vec<Frame>, unmatched: &str) -> Tree {
    matched(
        json!({"except": ["run"], "unmatched": unmatched}),
        vec![("a", Tree::Frames(a)), ("b", Tree::Frames(b))],
    )
    .unwrap()
}

fn with_expressions(expressions: Value, tree: Tree) -> Result<Tree> {
    let inputs = compute::MatchInputs {
        names: vec!["a".to_owned(), "b".to_owned()],
        columns: BTreeMap::new(),
    };
    compute::combine(
        "c",
        &node::<CombineNode>(json!({"expressions": expressions})),
        tree,
        Some(&inputs),
        &catalogue(),
    )
}

#[test]
fn combine_expressions_compute_per_point_with_units() {
    let tree = pairs(
        vec![
            case_frame("runs", "r1", num(10), &[Some(10.0)]),
            case_frame("runs", "r1", num(20), &[None]),
            case_frame("runs", "r1", num(30), &[Some(30.0)]),
        ],
        vec![
            case_frame("ref", "ref", num(10), &[Some(4.0)]),
            case_frame("ref", "ref", num(20), &[Some(5.0)]),
        ],
        "keep",
    );
    let out = with_expressions(
        json!([
            {"name": "diff", "expression": "a - b"},
            {"name": "ratio", "expression": "a / b"},
            {"name": "pct", "expression": "a / b", "unit": "%"},
            {"name": "in_s", "expression": "max(a, b)", "unit": "s"}
        ]),
        tree,
    )
    .unwrap();
    let fs = frames(&out);
    assert_eq!(fs.len(), 3);
    assert_eq!(
        columns(&fs[0]),
        [
            ("a", "ms"),
            ("b", "ms"),
            ("diff", "ms"),
            ("ratio", "ms/ms"),
            ("pct", "%"),
            ("in_s", "s")
        ]
    );
    assert!(catalogue().convertible("ms/ms", ""));
    assert_eq!(col(&fs[0], "diff"), [Some(6.0)]);
    assert_eq!(col(&fs[0], "ratio"), [Some(2.5)]);
    assert!(close(col(&fs[0], "pct")[0], 250.0));
    assert!(close(col(&fs[0], "in_s")[0], 0.01));
    // No value and a missing partner give no value, in the same columns.
    assert_eq!(col(&fs[1], "diff"), [None]);
    assert_eq!(columns(&fs[2]), columns(&fs[0]));
    assert_eq!(col(&fs[2], "b"), [None]);
    assert_eq!(col(&fs[2], "ratio"), [None]);
    // a's attributes, and the partner's under its name.
    assert_eq!(attr(&fs[0], "run.id"), Some(&text("r1")));
    assert_eq!(attr(&fs[0], "b.run.id"), Some(&text("ref")));
    assert_eq!(attr(&fs[0], "b.source"), Some(&text("ref")));
    assert_eq!(attr(&fs[2], "b.run.id"), None);
    assert!(fs[0].input.is_none());
}

#[test]
fn combine_expressions_read_columns_and_attributes_of_the_inputs() {
    let reduced_a = reduced(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![case_frame(
            "runs",
            "r1",
            num(10),
            &[Some(4.0), Some(8.0)],
        )]),
    )
    .unwrap();
    let Tree::Frames(a) = reduced_a else { panic!() };
    let mut b = case_frame("ref", "ref", num(10), &[Some(3.0)]);
    b.attrs
        .insert("sample.repetition".into(), Attr::new(num(2)));
    let tree = matched(
        json!({"except": ["run", "sample"]}),
        vec![("a", Tree::Frames(a)), ("b", Tree::Frames(vec![b]))],
    )
    .unwrap();
    let out = with_expressions(
        json!([
            {"name": "ratio", "expression": "a.mean / b"},
            {"name": "per_rep", "expression": "b / b.sample.repetition"},
            {"name": "area", "expression": "param.w * b.param.w"}
        ]),
        tree.clone(),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(
        columns(c),
        [
            ("a.mean", "ms"),
            ("a.n", ""),
            ("a.n_no_value", ""),
            ("b", "ms"),
            ("ratio", "ms/ms"),
            ("per_rep", "ms"),
            ("area", "px*px")
        ]
    );
    assert_eq!(col(c, "ratio"), [Some(2.0)]);
    assert_eq!(col(c, "per_rep"), [Some(1.5)]);
    assert_eq!(col(c, "area"), [Some(100.0)]);
    assert_eq!(attr(c, "b.sample.repetition"), Some(&num(2)));

    // No expressions: the inputs side by side.
    let out = with_expressions(json!([]), tree.clone()).unwrap();
    assert_eq!(
        columns(&frames(&out)[0]),
        [
            ("a.mean", "ms"),
            ("a.n", ""),
            ("a.n_no_value", ""),
            ("b", "ms")
        ]
    );

    // Units that do not fit, and names that are not there, are errors.
    let err = with_expressions(
        json!([{"name": "e", "expression": "b + param.w"}]),
        tree.clone(),
    )
    .unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("expressions[0].expression"));
    assert!(err.span.is_some());
    let err =
        with_expressions(json!([{"name": "e", "expression": "1 + c"}]), tree.clone()).unwrap_err();
    assert!(err.message.contains("unknown name c"), "{err}");
    assert_eq!(err.span, Some(crate::dsl::Span::new(4, 5)));
    let err =
        with_expressions(json!([{"name": "e", "expression": "a + 1"}]), tree.clone()).unwrap_err();
    assert!(
        err.message
            .contains("a has the columns mean, n, n_no_value"),
        "{err}"
    );
    let err = with_expressions(json!([{"name": "e", "expression": "b.mean"}]), tree).unwrap_err();
    assert!(err.message.contains("b has no column mean"), "{err}");
}

#[test]
fn combine_expressions_apply_a_scalar_to_every_point_of_a_series() {
    let a = series_frame(
        &[("run.id", text("r1"))],
        "s",
        &[0.0, 1.0],
        &[Some(30.0), Some(60.0)],
    );
    let mut b = frame(&[("run.id", text("ref"))], &[Some(30.0)]);
    b.columns[0].unit = "fps".into();
    let out = with_expressions(
        json!([{"name": "rel", "expression": "a / b", "unit": "%"}]),
        pairs(vec![a], vec![b], "drop"),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(
        columns(c),
        [("x", "s"), ("a", "fps"), ("b", "fps"), ("rel", "%")]
    );
    assert_eq!(col(c, "b"), [Some(30.0), Some(30.0)]);
    assert!(close(col(c, "rel")[1], 200.0));
}

#[test]
fn the_trend_of_the_adr_runs_end_to_end() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "lines", "type": "group", "input": "runs", "by": ["case", "device.name"]},
        {"id": "per_run", "type": "group", "input": "lines", "by": ["run.id"]},
        {"id": "samples", "type": "concatenate", "input": "per_run"},
        {"id": "stats", "type": "reduce", "input": "samples",
         "functions": ["mean", "ci_low", "ci_high"]},
        {"id": "chart", "type": "line", "input": "stats",
         "x": "run.started_at", "y": "mean", "low": "ci_low", "high": "ci_high"}
    ]));
    let s = |run: &str, day: u32, case: &str, rep: i32, y: f64| {
        frame(
            &[
                ("run.id", text(run)),
                ("run.started_at", time(day)),
                ("scenario.name", text(case)),
                ("scenario.version", num(1)),
                ("device.name", text("d1")),
                ("sample.repetition", num(rep)),
            ],
            &[Some(y)],
        )
    };
    let source = Tree::Frames(vec![
        s("r1", 1, "open", 1, 1.0),
        s("r1", 1, "open", 2, 3.0),
        s("r2", 2, "open", 1, 2.0),
        s("r2", 2, "open", 2, 4.0),
        s("r1", 1, "save", 1, 10.0),
    ]);
    let out = run(
        &spec,
        &graph,
        BTreeMap::from([("runs".to_owned(), source)]),
        None,
        &catalogue(),
        usize::MAX,
    )
    .unwrap();
    let [open, save] = frames(&out) else {
        panic!("one frame per case and device")
    };
    assert_eq!(attr(open, "scenario.name"), Some(&text("open")));
    assert_eq!(col(open, "mean"), [Some(2.0), Some(3.0)]);
    assert_eq!(col(open, "n"), [Some(2.0), Some(2.0)]);
    let s = crate::stats::summarize(&[Some(1.0), Some(3.0)], &[], 0.95);
    assert_eq!(col(open, "ci_low")[0], s.ci.map(|c| c.0));
    assert_eq!(col(open, "ci_high")[0], s.ci.map(|c| c.1));
    assert_eq!(open.point_attr(1, "run.started_at").unwrap().value, time(2));
    assert_eq!(col(save, "mean"), [Some(10.0)]);
    assert_eq!(col(save, "ci_low"), [None]);
}

#[test]
fn a_ratio_to_a_reference_runs_end_to_end() {
    let stats = |id: &str, input: &str| {
        [
            json!({"id": format!("{id}_lines"), "type": "group", "input": input,
                   "by": ["case", "device.name"]}),
            json!({"id": format!("{id}_runs"), "type": "group", "input": format!("{id}_lines"),
                   "by": ["run.id"]}),
            json!({"id": format!("{id}_samples"), "type": "concatenate",
                   "input": format!("{id}_runs")}),
            json!({"id": format!("{id}_stats"), "type": "reduce",
                   "input": format!("{id}_samples"), "functions": ["mean"]}),
        ]
    };
    let mut nodes = vec![
        json!({"id": "runs", "type": "source", "metric": "load_time"}),
        json!({"id": "ref", "type": "source", "metric": "load_time"}),
    ];
    nodes.extend(stats("a", "runs"));
    nodes.extend(stats("b", "ref"));
    nodes.extend([
        json!({"id": "pairs", "type": "match", "inputs": {"a": "a_stats", "b": "b_stats"},
               "except": ["run"]}),
        json!({"id": "ratio", "type": "combine", "input": "pairs",
               "expressions": [{"name": "ratio", "expression": "a.mean / b.mean"}]}),
        json!({"id": "chart", "type": "line", "input": "ratio",
               "x": "run.started_at", "y": "ratio"}),
    ]);
    let (spec, graph) = bound_spec(Value::Array(nodes));
    let s = |run: &str, day: u32, case: &str, y: f64| {
        frame(
            &[
                ("run.id", text(run)),
                ("run.started_at", time(day)),
                ("scenario.name", text(case)),
                ("device.name", text("d1")),
            ],
            &[Some(y)],
        )
    };
    let runs = Tree::Frames(vec![
        s("r1", 1, "open", 4.0),
        s("r1", 1, "open", 6.0),
        s("r2", 2, "open", 10.0),
        s("r1", 1, "save", 3.0),
        s("r1", 1, "close", 3.0),
    ]);
    let reference = Tree::Frames(vec![
        s("r0", 1, "open", 2.0),
        s("r0", 1, "open", 3.0),
        s("r0", 1, "save", 1.5),
    ]);
    let out = run(
        &spec,
        &graph,
        BTreeMap::from([("runs".to_owned(), runs), ("ref".to_owned(), reference)]),
        None,
        &catalogue(),
        usize::MAX,
    )
    .unwrap();
    // close has no reference and is dropped.
    let [open, save] = frames(&out) else {
        panic!("{out:#?}")
    };
    assert_eq!(attr(open, "scenario.name"), Some(&text("open")));
    assert_eq!(col(open, "ratio"), [Some(2.0), Some(4.0)]);
    assert!(catalogue().convertible(&open.columns[open.column("ratio").unwrap()].unit, ""));
    assert_eq!(open.point_attr(1, "run.id").unwrap().value, text("r2"));
    assert_eq!(attr(open, "b.run.id"), Some(&text("r0")));
    assert_eq!(col(save, "ratio"), [Some(2.0)]);
}

// Cross-review of #29 ----------------------------------------------------------------------

#[test]
fn resample_puts_x_in_different_units_on_one_grid() {
    let fs = vec![
        series_frame(&[], "s", &[0.0, 1.0], &[Some(0.0), Some(10.0)]),
        series_frame(&[], "ms", &[0.0, 1000.0], &[Some(0.0), Some(10.0)]),
    ];
    let out = resampled(json!({"points": 3}), Tree::Frames(fs.clone())).unwrap();
    for f in frames(&out) {
        assert_eq!(f.columns[0], Column::x("s", "time"));
        assert_eq!(col(f, "x"), [Some(0.0), Some(0.5), Some(1.0)]);
        assert_eq!(col(f, "y"), [Some(0.0), Some(5.0), Some(10.0)]);
    }
    let mut bytes = fs;
    bytes[1].columns[0].unit = "B".into();
    let err = resampled(json!({"points": 3}), Tree::Frames(bytes)).unwrap_err();
    assert!(err.message.contains("cannot be converted"), "{err}");
}

#[test]
fn nodes_that_make_points_stay_within_the_budget() {
    let two = || series_frame(&[], "s", &[0.0, 1.0], &[Some(0.0), Some(1.0)]);
    let tree = Tree::Frames(vec![two(), two()]);
    let spec = node::<ResampleNode>(json!({"points": 60}));
    let err = compute::resample("g", &spec, tree.clone(), &catalogue(), 100).unwrap_err();
    assert!(err.message.contains("budget"), "{err}");
    assert!(compute::resample("g", &spec, tree, &catalogue(), 120).is_ok());

    // A partner of several frames counts for each.
    let a = Tree::Frames(
        (0..3)
            .map(|i| case_frame("runs", &format!("r{i}"), num(10), &[Some(1.0)]))
            .collect(),
    );
    let b = Tree::Frames(vec![case_frame("ref", "ref", num(10), &[Some(1.0); 10])]);
    let spec = node::<MatchNode>(json!({"except": ["run"]}));
    let inputs = || vec![("a".to_owned(), a.clone()), ("b".to_owned(), b.clone())];
    let err = compute::match_frames("m", &spec, inputs(), 32).unwrap_err();
    assert!(err.message.contains("budget"), "{err}");
    assert!(compute::match_frames("m", &spec, inputs(), 33).is_ok());

    // A run counts its sources and fails at the node that goes over.
    let (spec, graph) = bound_spec(json!([
        {"id": "fps", "type": "source", "metric": "series.fps"},
        {"id": "grid", "type": "resample", "input": "fps", "points": 60},
        {"id": "chart", "type": "line", "input": "grid", "x": "x", "y": "y"}
    ]));
    let sources = || BTreeMap::from([("fps".to_owned(), Tree::Frames(vec![two(), two()]))]);
    let err = run(&spec, &graph, sources(), None, &catalogue(), 100).unwrap_err();
    assert_eq!(err.node.as_deref(), Some("grid"));
    let out = run(&spec, &graph, sources(), None, &catalogue(), 124).unwrap();
    assert_eq!(frames(&out).len(), 2);
}

#[test]
fn combine_reads_and_keeps_the_point_attributes_of_a_partner() {
    let a = frame(&[("run.id", text("r1"))], &[Some(10.0), Some(20.0)]);
    let mut b = frame(&[("run.id", text("ref"))], &[Some(2.0), Some(4.0)]);
    let rep = |r: i32| Attributes::from([("sample.repetition".to_owned(), Attr::new(num(r)))]);
    b.point_attrs = Some(vec![rep(1), rep(2)]);
    let out = with_expressions(
        json!([{"name": "e", "expression": "b * b.sample.repetition"}]),
        pairs(vec![a], vec![b], "drop"),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(col(c, "e"), [Some(2.0), Some(8.0)]);
    assert_eq!(
        c.point_attr(1, "b.sample.repetition").unwrap().value,
        num(2)
    );
}

#[test]
fn combine_functions_share_only_what_every_frame_has_at_a_point() {
    // d1 has points for r1 and r2; d2 one point for r1, which applies to
    // both points.
    let mut d1 = frame(&[("device.name", text("d1"))], &[Some(1.0), Some(3.0)]);
    let run = |r: &str| Attributes::from([("run.id".to_owned(), Attr::new(text(r)))]);
    d1.point_attrs = Some(vec![run("r1"), run("r2")]);
    let d2 = frame(
        &[("device.name", text("d2")), ("run.id", text("r1"))],
        &[Some(5.0)],
    );
    let out = combined(json!({"functions": ["mean"]}), Tree::Frames(vec![d1, d2])).unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(col(c, "mean"), [Some(3.0), Some(4.0)]);
    assert_eq!(c.point_attr(0, "run.id").unwrap().value, text("r1"));
    assert!(c.point_attr(1, "run.id").is_none());
}

#[test]
fn combining_empty_series_keeps_x() {
    let out = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![series_frame(&[], "s", &[], &[])]),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(c.columns[0], Column::x("s", "time"));
    assert!(c.is_empty());
}

#[test]
fn an_expression_without_a_value_keeps_its_unit() {
    let mut a = frame(&[("run.id", text("r1"))], &[None]);
    a.attrs
        .insert("sample.repetition".into(), Attr::new(num(2)));
    let b = frame(&[("run.id", text("ref"))], &[Some(1.0)]);
    let out = with_expressions(
        json!([{"name": "e", "expression": "a * sample.repetition"}]),
        matched(
            json!({"except": ["run", "sample"]}),
            vec![("a", Tree::Frames(vec![a])), ("b", Tree::Frames(vec![b]))],
        )
        .unwrap(),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(col(c, "e"), [None]);
    assert_eq!(c.columns[c.column("e").unwrap()].unit, "ms");
}

#[test]
fn a_merge_inside_a_match_group_keeps_the_input() {
    // Each pair is grouped by source and each one-frame subgroup is
    // concatenated: the match level stays, and so do the input names.
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"]},
        {"id": "by_source", "type": "group", "input": "pairs", "by": ["source"]},
        {"id": "merged", "type": "concatenate", "input": "by_source"},
        {"id": "diff", "type": "combine", "input": "merged",
         "expressions": [{"name": "d", "expression": "a - b"}]},
        {"id": "chart", "type": "table", "input": "diff", "columns": ["d"]}
    ]));
    let sources = BTreeMap::from([
        (
            "runs".to_owned(),
            Tree::Frames(vec![case_frame("runs", "r1", num(10), &[Some(5.0)])]),
        ),
        (
            "ref".to_owned(),
            Tree::Frames(vec![case_frame("ref", "r0", num(10), &[Some(3.0)])]),
        ),
    ]);
    let out = run(&spec, &graph, sources, None, &catalogue(), usize::MAX).unwrap();
    assert_eq!(col(&frames(&out)[0], "d"), [Some(2.0)]);
}

#[test]
fn statistics_beyond_the_range_of_f64_are_no_value() {
    let out = reduced(
        json!({"functions": ["mean", "sum", "stddev"]}),
        Tree::Frames(vec![frame(&[], &[Some(1e308), Some(1e308), Some(-1e308)])]),
    )
    .unwrap();
    let f = &frames(&out)[0];
    // An overflow on the way does not lose the mean or the sum.
    assert!(close(col(f, "mean")[0], 1e308 / 3.0));
    assert_eq!(col(f, "sum"), [Some(1e308)]);
    assert!(col(f, "stddev")[0].is_some_and(f64::is_finite));
    assert_eq!(col(f, "n"), [Some(3.0)]);
}

// Cross-review of #29, round 2 -------------------------------------------------------------

#[test]
fn an_input_without_partners_keeps_its_columns() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"], "unmatched": "keep"},
        {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
        {"id": "stats", "type": "reduce", "input": "both", "column": "b",
         "functions": ["mean"]},
        {"id": "chart", "type": "table", "input": "stats", "columns": ["mean"]}
    ]));
    let sources = BTreeMap::from([
        (
            "runs".to_owned(),
            Tree::Frames(vec![case_frame("runs", "r1", num(10), &[Some(5.0)])]),
        ),
        (
            "ref".to_owned(),
            Tree::Frames(vec![case_frame("ref", "r0", num(20), &[Some(3.0)])]),
        ),
    ]);
    let out = run(&spec, &graph, sources, None, &catalogue(), usize::MAX).unwrap();
    let f = &frames(&out)[0];
    assert_eq!(col(f, "mean"), [None]);
    assert_eq!(col(f, "n_no_value"), [Some(1.0)]);
    assert_eq!(f.columns[0], Column::new("mean", "ms"));
}

#[test]
fn copies_for_several_readers_count_against_the_budget() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "a", "type": "filter", "input": "runs",
         "conditions": [{"attribute": "run.id", "op": "=", "value": "none"}]},
        {"id": "b", "type": "filter", "input": "runs",
         "conditions": [{"attribute": "run.id", "op": "=", "value": "none"}]},
        {"id": "m", "type": "match", "inputs": {"a": "a", "b": "b"}},
        {"id": "chart", "type": "table", "input": "m", "columns": ["y"]}
    ]));
    let sources = || {
        BTreeMap::from([(
            "runs".to_owned(),
            Tree::Frames(vec![sample("r1", 1, &[1.0; 10])]),
        )])
    };
    // The first filter gets a copy while the source is still needed.
    let err = run(&spec, &graph, sources(), None, &catalogue(), 19).unwrap_err();
    assert_eq!(err.node.as_deref(), Some("a"));
    assert!(run(&spec, &graph, sources(), None, &catalogue(), 20).is_ok());
}

#[test]
fn a_point_that_overrides_a_shared_attribute_is_not_shared() {
    let mut a = frame(
        &[("run.id", text("r1")), ("device.name", text("d1"))],
        &[Some(1.0), Some(2.0)],
    );
    a.point_attrs = Some(vec![
        Attributes::new(),
        Attributes::from([("run.id".to_owned(), Attr::new(text("r2")))]),
    ]);
    let b = frame(
        &[("run.id", text("r1")), ("device.name", text("d2"))],
        &[Some(3.0), Some(4.0)],
    );
    let out = combined(json!({"functions": ["mean"]}), Tree::Frames(vec![a, b])).unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(attr(c, "run.id"), None);
    assert_eq!(c.point_attr(0, "run.id").unwrap().value, text("r1"));
    assert!(c.point_attr(1, "run.id").is_none());
}

#[test]
fn an_empty_series_with_a_scalar_keeps_x() {
    let out = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![series_frame(&[], "s", &[], &[]), {
            let mut f = frame(&[], &[Some(2.0)]);
            f.columns[0].unit = "fps".into();
            f
        }]),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(c.columns[0], Column::x("s", "time"));
    assert_eq!(col(c, "x"), [None]);
    assert_eq!(col(c, "mean"), [Some(2.0)]);
    // As with a frame of several points without x.
    let out = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![series_frame(&[], "s", &[], &[]), {
            let mut f = frame(&[], &[Some(2.0), Some(4.0)]);
            f.columns[0].unit = "fps".into();
            f
        }]),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(c.columns[0], Column::x("s", "time"));
    assert_eq!(col(c, "x"), [None, None]);
    assert_eq!(col(c, "mean"), [Some(2.0), Some(4.0)]);
}

#[test]
fn an_expression_over_no_points_keeps_its_unit() {
    let empty = |run: &str| {
        let mut f = frame(&[("run.id", text(run))], &[]);
        f.attrs
            .insert("sample.repetition".into(), Attr::new(num(2)));
        f
    };
    let tree = matched(
        json!({"except": ["run", "sample"]}),
        vec![
            ("a", Tree::Frames(vec![empty("r1")])),
            ("b", Tree::Frames(vec![empty("r0")])),
        ],
    )
    .unwrap();
    let out = with_expressions(
        json!([{"name": "e", "expression": "a * sample.repetition"}]),
        tree,
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert!(c.is_empty());
    assert_eq!(c.columns[c.column("e").unwrap()].unit, "ms");
}

#[test]
fn a_merge_inside_a_match_keeps_the_names_of_inputs_without_frames() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"], "unmatched": "keep"},
        {"id": "by_source", "type": "group", "input": "pairs", "by": ["source"]},
        {"id": "merged", "type": "concatenate", "input": "by_source"},
        {"id": "diff", "type": "combine", "input": "merged",
         "expressions": [{"name": "d", "expression": "a - b"}]},
        {"id": "chart", "type": "table", "input": "diff", "columns": ["d"]}
    ]));
    let sources = BTreeMap::from([
        (
            "runs".to_owned(),
            Tree::Frames(vec![case_frame("runs", "r1", num(10), &[Some(5.0)])]),
        ),
        ("ref".to_owned(), Tree::Frames(vec![])),
    ]);
    let out = run(&spec, &graph, sources, None, &catalogue(), usize::MAX).unwrap();
    assert_eq!(col(&frames(&out)[0], "d"), [None]);
}

#[test]
fn a_reduce_inside_a_match_gives_the_inputs_its_columns() {
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "load_time"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"]},
        {"id": "by_source", "type": "group", "input": "pairs", "by": ["source"]},
        {"id": "stats", "type": "reduce", "input": "by_source", "functions": ["mean"]},
        {"id": "diff", "type": "combine", "input": "stats",
         "expressions": [{"name": "d", "expression": "a.mean - b.mean"}]},
        {"id": "chart", "type": "table", "input": "diff", "columns": ["d"]}
    ]));
    let sources = BTreeMap::from([
        (
            "runs".to_owned(),
            Tree::Frames(vec![case_frame(
                "runs",
                "r1",
                num(10),
                &[Some(5.0), Some(7.0)],
            )]),
        ),
        (
            "ref".to_owned(),
            Tree::Frames(vec![case_frame("ref", "r0", num(10), &[Some(3.0)])]),
        ),
    ]);
    let out = run(&spec, &graph, sources, None, &catalogue(), usize::MAX).unwrap();
    let f = &frames(&out)[0];
    assert_eq!(col(f, "d"), [Some(3.0)]);
    assert_eq!(f.columns[f.column("d").unwrap()].unit, "ms");
}

#[test]
fn resample_keeps_the_last_value_at_the_end_of_the_grid() {
    let f = series_frame(&[], "s", &[0.0, 1.0], &[Some(0.0), Some(10.0)]);
    let out = resampled(
        json!({"points": 4, "from": 0.2, "to": 1}),
        Tree::Frames(vec![f]),
    )
    .unwrap();
    let f = &frames(&out)[0];
    assert_eq!(col(f, "x")[3], Some(1.0));
    assert_eq!(col(f, "y")[3], Some(10.0));
}

#[test]
fn x_of_different_dimensions_is_not_one_axis() {
    let mut cpu = series_frame(&[], "s", &[0.0, 1.0], &[Some(1.0), Some(2.0)]);
    cpu.columns[0].dimension = "cpu time".into();
    let wall = series_frame(&[], "s", &[0.0, 1.0], &[Some(1.0), Some(2.0)]);
    let err = resampled(
        json!({"points": 3}),
        Tree::Frames(vec![wall.clone(), cpu.clone()]),
    )
    .unwrap_err();
    assert!(err.message.contains("not one axis"), "{err}");
    let err = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![wall, cpu]),
    )
    .unwrap_err();
    assert!(err.message.contains("cpu time"), "{err}");
    // Also series without points, whose x the result keeps.
    let mut cpu = series_frame(&[], "s", &[], &[]);
    cpu.columns[0].dimension = "cpu time".into();
    let wall = series_frame(&[], "s", &[], &[]);
    let err = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![wall.clone(), cpu]),
    )
    .unwrap_err();
    assert!(err.message.contains("not one axis"), "{err}");
    let bytes = series_frame(&[], "B", &[], &[]);
    let err = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![wall, bytes]),
    )
    .unwrap_err();
    assert!(err.message.contains("cannot be converted"), "{err}");
}

#[test]
fn x_in_the_same_unit_must_be_equal() {
    let a = series_frame(&[], "s", &[1e9, 1e9 + 1.0], &[Some(1.0), Some(2.0)]);
    let b = series_frame(&[], "s", &[1e9 + 0.5, 1e9 + 1.5], &[Some(1.0), Some(2.0)]);
    let err = combined(json!({"functions": ["mean"]}), Tree::Frames(vec![a, b])).unwrap_err();
    assert!(err.message.contains("x values differ"), "{err}");
}

#[test]
fn units_are_checked_within_a_group() {
    let in_unit = |source: &str, run: &str, w: i32, unit: &str| {
        let mut f = case_frame(source, run, num(w), &[Some(4.0)]);
        f.columns[0].unit = unit.into();
        f
    };
    let tree = pairs(
        vec![
            in_unit("runs", "r1", 10, "s"),
            in_unit("runs", "r1", 20, "B"),
        ],
        vec![in_unit("ref", "r0", 20, "B")],
        "keep",
    );
    let out = with_expressions(json!([{"name": "d", "expression": "a - b"}]), tree).unwrap();
    let fs = frames(&out);
    assert_eq!(col(&fs[0], "d"), [None]);
    assert_eq!(col(&fs[1], "d"), [Some(0.0)]);
    assert_eq!(fs[1].columns[fs[1].column("d").unwrap()].unit, "B");
}

#[test]
fn a_group_without_a_partner_keeps_the_units_it_has() {
    let in_unit = |source: &str, w: i32, unit: &str| {
        let mut f = case_frame(source, "r", num(w), &[Some(4.0)]);
        f.columns[0].unit = unit.into();
        f.attrs
            .insert("sample.repetition".into(), Attr::new(num(2)));
        f
    };
    let tree = pairs(
        vec![in_unit("runs", 10, "s"), in_unit("runs", 20, "ms")],
        vec![in_unit("ref", 10, "s")],
        "keep",
    );
    let out = with_expressions(
        json!([{"name": "e", "expression": "a * b.sample.repetition"}]),
        tree,
    )
    .unwrap();
    let fs = frames(&out);
    let unit = |f: &Frame| f.columns[f.column("e").unwrap()].unit.clone();
    assert_eq!(
        (col(&fs[0], "e"), unit(&fs[0])),
        (vec![Some(8.0)], "s".into())
    );
    // No partner, so no value; the unit is that of a here, not of a in
    // another group.
    assert_eq!((col(&fs[1], "e"), unit(&fs[1])), (vec![None], "ms".into()));
}

#[test]
fn a_match_group_names_the_attributes_only_names() {
    let a = Tree::Frames(vec![frame(&[("run.id", text("r1"))], &[Some(1.0)])]);
    let b = Tree::Frames(vec![frame(&[("run.id", text("r0"))], &[Some(2.0)])]);
    let out = matched(
        json!({"only": ["param.codec", "case"]}),
        vec![("a", a), ("b", b)],
    )
    .unwrap();
    assert_eq!(groups(&out)[0].key, [("param.codec".to_owned(), None)]);
}

// Cross-review of #29, round 3 -------------------------------------------------------------

#[test]
fn a_series_of_one_point_resamples_without_gaps() {
    let f = series_frame(&[], "s", &[100.0], &[Some(7.0)]);
    let out = resampled(json!({}), Tree::Frames(vec![f])).unwrap();
    let f = &frames(&out)[0];
    assert_eq!(f.len(), 200);
    assert!(col(f, "x").iter().all(|x| *x == Some(100.0)));
    assert!(col(f, "y").iter().all(|y| *y == Some(7.0)));
}

#[test]
fn x_after_a_conversion_is_equal_only_up_to_rounding() {
    let a = series_frame(&[], "s", &[1e9, 1e9 + 1.0], &[Some(1.0), Some(2.0)]);
    let b = series_frame(
        &[],
        "ms",
        &[1e12 + 500.0, 1e12 + 1500.0],
        &[Some(1.0), Some(2.0)],
    );
    let err = combined(
        json!({"functions": ["mean"]}),
        Tree::Frames(vec![a.clone(), b]),
    )
    .unwrap_err();
    assert!(err.message.contains("x values differ"), "{err}");
    let b = series_frame(&[], "ms", &[1e12, 1e12 + 1000.0], &[Some(1.0), Some(2.0)]);
    assert!(combined(json!({"functions": ["mean"]}), Tree::Frames(vec![a, b])).is_ok());
    // A conversion that overflows is not equal to anything.
    let ms = series_frame(&[], "ms", &[20.0], &[Some(1.0)]);
    let s = series_frame(&[], "s", &[1e308], &[Some(2.0)]);
    let err = combined(json!({"functions": ["mean"]}), Tree::Frames(vec![ms, s])).unwrap_err();
    assert!(err.message.contains("x values differ"), "{err}");
}

#[test]
fn the_partners_of_a_combine_replace_those_of_an_earlier_one() {
    let with_rep = |run: &str, rep: i32, y: f64| {
        let mut f = frame(&[("run.id", text(run))], &[Some(y)]);
        f.attrs
            .insert("sample.repetition".into(), Attr::new(num(rep)));
        f
    };
    let key = json!({"except": ["run", "sample"]});
    let first = matched(
        key.clone(),
        vec![
            ("a", Tree::Frames(vec![with_rep("r1", 0, 1.0)])),
            ("b", Tree::Frames(vec![with_rep("r0", 1, 2.0)])),
        ],
    )
    .unwrap();
    let Tree::Frames(once) = with_expressions(json!([]), first).unwrap() else {
        panic!()
    };
    // a now carries b.sample.repetition = 1; a new b has 3.
    let mut a = once[0].clone();
    a.columns[0].name = "y".into();
    a.values.truncate(1);
    a.columns.truncate(1);
    let second = matched(
        key,
        vec![
            ("a", Tree::Frames(vec![a])),
            ("b", Tree::Frames(vec![with_rep("r9", 3, 4.0)])),
        ],
    )
    .unwrap();
    let out = with_expressions(
        json!([{"name": "rep", "expression": "b.sample.repetition"}]),
        second,
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(col(c, "rep"), [Some(3.0)]);
    assert_eq!(
        c.point_attr(0, "b.sample.repetition").unwrap().value,
        num(3)
    );
}

#[test]
fn a_partner_without_frames_here_still_gives_x() {
    let a = frame(
        &[("run.id", text("r1")), ("param.w", num(10))],
        &[Some(1.0)],
    );
    let b = series_frame(
        &[("run.id", text("r0")), ("param.w", num(20))],
        "s",
        &[0.0, 1.0],
        &[Some(1.0), Some(2.0)],
    );
    let (spec, graph) = bound_spec(json!([
        {"id": "runs", "type": "source", "metric": "load_time"},
        {"id": "ref", "type": "source", "metric": "series.fps"},
        {"id": "pairs", "type": "match", "inputs": {"a": "runs", "b": "ref"},
         "except": ["run"], "unmatched": "keep"},
        {"id": "both", "type": "combine", "input": "pairs", "expressions": []},
        {"id": "chart", "type": "table", "input": "both", "columns": ["a"]}
    ]));
    let sources = BTreeMap::from([
        ("runs".to_owned(), Tree::Frames(vec![a])),
        ("ref".to_owned(), Tree::Frames(vec![b])),
    ]);
    let out = run(&spec, &graph, sources, None, &catalogue(), usize::MAX).unwrap();
    let f = &frames(&out)[0];
    assert_eq!(f.columns[0], Column::x("s", "time"));
    assert_eq!(col(f, "x"), [None]);
    assert_eq!(col(f, "b"), [None]);
}

#[test]
fn a_point_shares_the_attributes_of_the_frames_with_a_value_there() {
    let r1 = frame(&[("run.id", text("r1"))], &[Some(1.0), None]);
    let r2 = frame(&[("run.id", text("r2"))], &[None, Some(2.0)]);
    let out = combined(json!({"functions": ["mean"]}), Tree::Frames(vec![r1, r2])).unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(c.point_attr(0, "run.id").unwrap().value, text("r1"));
    assert_eq!(c.point_attr(1, "run.id").unwrap().value, text("r2"));
    // A value that the conversion takes out of range is no value, also for
    // the attributes: only r1 gives the mean.
    let r1 = frame(&[("run.id", text("r1"))], &[Some(1.0)]);
    let mut r2 = frame(&[("run.id", text("r2"))], &[Some(1e308)]);
    r2.columns[0].unit = "s".into();
    let out = combined(json!({"functions": ["mean"]}), Tree::Frames(vec![r1, r2])).unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(col(c, "mean"), [Some(1.0)]);
    assert_eq!(col(c, "n_no_value"), [Some(1.0)]);
    assert_eq!(c.point_attr(0, "run.id").unwrap().value, text("r1"));
}

#[test]
fn a_group_without_points_takes_units_from_its_frames() {
    let tree = pairs(
        vec![case_frame("runs", "r1", num(10), &[])],
        vec![case_frame("ref", "r0", num(10), &[])],
        "drop",
    );
    let out = with_expressions(
        json!([{"name": "e", "expression": "a * param.w"}]),
        tree.clone(),
    )
    .unwrap();
    let [c] = frames(&out) else { panic!() };
    assert_eq!(c.columns[c.column("e").unwrap()].unit, "ms*px");
    // Their units are checked as those of a point would be.
    let err =
        with_expressions(json!([{"name": "e", "expression": "a + param.w"}]), tree).unwrap_err();
    assert_eq!(err.setting.as_deref(), Some("expressions[0].expression"));
}
