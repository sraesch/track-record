//! The nodes that arrange frames without computing new values (ADR 0022,
//! "Nodes"): filter, group, flatten, sort, select_latest and concatenate.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{Result, bound, map_frames, map_innermost, merge_innermost, prune};
use crate::chart::attributes::Key;
use crate::chart::frame::{
    Attr, AttrValue, Attributes, Column, Frame, Group, Tree, cmp_missing_last, common_attributes,
};
use crate::chart::problem::Problem;
use crate::chart::spec::{
    FilterNode, FlattenLevels, FlattenNode, GroupNode, On, Op, SelectLatestNode, SortNode,
    SortTarget,
};

// filter ---------------------------------------------------------------------------------

/// A condition with its bound value.
struct Cond<'a> {
    attribute: &'a str,
    op: Op,
    value: Option<&'a Value>,
}

/// Keeps the frames, or the points, that meet all conditions. Groups that
/// become empty are removed; with `on: points` also the frames.
pub fn filter(id: &str, n: &FilterNode, tree: Tree) -> Result<Tree> {
    let conds = n
        .conditions
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let value = match &c.value {
                Some(s) => Some(bound(id, &format!("conditions[{i}].value"), s)?),
                None => None,
            };
            Ok(Cond {
                attribute: &c.attribute,
                op: c.op,
                value,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let tree = match n.on {
        On::Frames => map_frames(tree, &mut |mut frames| {
            frames.retain(|f| {
                conds
                    .iter()
                    .all(|c| meets(c, f.attrs.get(c.attribute).map(|a| &a.value)))
            });
            Ok(frames)
        })?,
        On::Points => map_frames(tree, &mut |mut frames| {
            for f in &mut frames {
                let keep: Vec<usize> = (0..f.len())
                    .filter(|&i| {
                        conds.iter().all(|c| match f.column(c.attribute) {
                            Some(col) => {
                                let v = f.values[col][i].and_then(AttrValue::number);
                                meets(c, v.as_ref())
                            }
                            None => meets(c, f.point_attr(i, c.attribute).map(|a| &a.value)),
                        })
                    })
                    .collect();
                if keep.len() < f.len() {
                    f.select_points(&keep);
                }
            }
            Ok(frames)
        })?,
    };
    Ok(prune(tree, n.on == On::Points))
}

/// True if `value` meets the condition. A missing attribute and no value
/// never meet one, `!=` and `not in` included. On a list attribute
/// (`run.tags`), `=` means it contains the value, `in` that it contains one
/// of the values, and `!=` and `not in` the opposite.
fn meets(c: &Cond, value: Option<&AttrValue>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let list = || c.value.and_then(Value::as_array).map_or(&[][..], |v| v);
    match c.op {
        Op::Exists => true,
        Op::Eq => c.value.is_some_and(|v| equals(value, v)),
        Op::Ne => c.value.is_some_and(|v| !equals(value, v)),
        Op::In => list().iter().any(|v| equals(value, v)),
        Op::NotIn => !list().iter().any(|v| equals(value, v)),
        Op::Lt | Op::Le | Op::Gt | Op::Ge => {
            let Some(o) = c.value.and_then(|v| compare(value, v)) else {
                return false;
            };
            match c.op {
                Op::Lt => o == Ordering::Less,
                Op::Le => o != Ordering::Greater,
                Op::Gt => o == Ordering::Greater,
                _ => o != Ordering::Less,
            }
        }
    }
}

/// `=` between an attribute and a JSON value of a condition. Values keep
/// their JSON type: `10` is not `"10"`. A time compares with a text in
/// RFC 3339.
fn equals(value: &AttrValue, json: &Value) -> bool {
    match value {
        AttrValue::List(items) => items.iter().any(|item| equals(item, json)),
        AttrValue::Time(t) => time(json).is_some_and(|j| j == *t),
        _ => AttrValue::from_json(json).is_some_and(|j| value.same(&j)),
    }
}

/// The order of an attribute against a JSON value: numbers with numbers,
/// times with texts in RFC 3339. `None` for anything else.
fn compare(value: &AttrValue, json: &Value) -> Option<Ordering> {
    match value {
        AttrValue::Number(_) if json.is_number() => {
            AttrValue::from_json(json).map(|j| value.total_cmp(&j))
        }
        AttrValue::Time(t) => time(json).map(|j| t.cmp(&j)),
        _ => None,
    }
}

fn time(json: &Value) -> Option<DateTime<Utc>> {
    json.as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc))
}

// group ----------------------------------------------------------------------------------

/// Splits the frames of every innermost group by a key: one more level.
/// The subgroups are ordered by their key values ([`AttrValue::total_cmp`],
/// frames without a value for an attribute last); the frames keep their
/// order.
pub fn group(id: &str, n: &GroupNode, tree: Tree) -> Result<Tree> {
    let by = bound(id, "by", &n.by)?;
    let except = match &n.except {
        Some(s) => bound(id, "except", s)?.as_slice(),
        None => &[],
    };
    let key = Key { by, except };
    map_innermost(tree, &mut |frames| Ok(Tree::Groups(split(&key, frames))))
}

fn split(key: &Key, frames: Vec<Frame>) -> Vec<Group> {
    // Each frame's key holds the attributes it has that the key includes,
    // and the attributes the key names: sparse, so that families with many
    // parameters or assets cost only what the frames carry.
    let keys: Vec<Vec<(String, Option<Attr>)>> = frames
        .iter()
        .map(|f| {
            key.names(f.attrs.keys().map(String::as_str).chain(key.named()))
                .into_iter()
                .map(|n| {
                    let v = f.attrs.get(&n).cloned();
                    (n, v)
                })
                .collect()
        })
        .collect();
    let cmp = |a: &[(String, Option<Attr>)], b: &[(String, Option<Attr>)]| cmp_keys(key, a, b);
    let mut order: Vec<usize> = (0..frames.len()).collect();
    // Stable: frames with the same key keep their order.
    order.sort_by(|&a, &b| cmp(&keys[a], &keys[b]));

    let mut frames: Vec<Option<Frame>> = frames.into_iter().map(Some).collect();
    let mut groups: Vec<Group> = Vec::new();
    let mut last: Option<usize> = None;
    for i in order {
        let frame = frames[i].take().expect("each frame once");
        match last {
            Some(l) if cmp(&keys[l], &keys[i]) == Ordering::Equal => {
                let Tree::Frames(members) = &mut groups.last_mut().expect("a group").tree else {
                    unreachable!("subgroups hold frames")
                };
                members.push(frame);
            }
            _ => {
                groups.push(Group {
                    key: keys[i].clone(),
                    tree: Tree::Frames(vec![frame]),
                });
                last = Some(i);
            }
        }
    }
    groups
}

/// Compares two sparse keys as if both listed every attribute of either,
/// a missing one without a value.
fn cmp_keys(key: &Key, a: &[(String, Option<Attr>)], b: &[(String, Option<Attr>)]) -> Ordering {
    let rank = |n: &str| {
        (
            key.rank(n).unwrap_or((usize::MAX, usize::MAX)),
            n.to_owned(),
        )
    };
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        let (x, y) = match (a.get(i), b.get(j)) {
            (Some(p), Some(q)) => match rank(&p.0).cmp(&rank(&q.0)) {
                Ordering::Equal => {
                    i += 1;
                    j += 1;
                    (p.1.as_ref(), q.1.as_ref())
                }
                Ordering::Less => {
                    i += 1;
                    (p.1.as_ref(), None)
                }
                Ordering::Greater => {
                    j += 1;
                    (None, q.1.as_ref())
                }
            },
            (Some(p), None) => {
                i += 1;
                (p.1.as_ref(), None)
            }
            (None, Some(q)) => {
                j += 1;
                (None, q.1.as_ref())
            }
            (None, None) => break,
        };
        let o = cmp_attrs(x, y);
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

/// Orders attributes by value, then unit, so that the same value in two
/// units (a parameter of `10 Mbit/s` and one of `10 kbit/s`) are two keys.
/// Missing ones go last.
pub(crate) fn cmp_attrs(a: Option<&Attr>, b: Option<&Attr>) -> Ordering {
    cmp_missing_last(a.map(|x| &x.value), b.map(|x| &x.value), false)
        .then_with(|| a.map(|x| x.unit.as_str()).cmp(&b.map(|x| x.unit.as_str())))
}

// flatten --------------------------------------------------------------------------------

/// Removes the innermost group level (or `levels` of them, or all). The
/// frames move up, in order.
pub fn flatten(n: &FlattenNode, tree: Tree) -> Tree {
    let depth = tree.depth();
    let remove = match n.levels.unwrap_or(FlattenLevels::Count(1)) {
        FlattenLevels::All => depth,
        FlattenLevels::Count(k) => (k as usize).min(depth),
    };
    flatten_to(tree, depth - remove)
}

fn flatten_to(tree: Tree, keep: usize) -> Tree {
    match tree {
        Tree::Groups(groups) if keep > 0 => Tree::Groups(
            groups
                .into_iter()
                .map(|g| Group {
                    key: g.key,
                    tree: flatten_to(g.tree, keep - 1),
                })
                .collect(),
        ),
        tree => {
            let mut frames = Vec::new();
            into_frames(tree, &mut frames);
            Tree::Frames(frames)
        }
    }
}

fn into_frames(tree: Tree, out: &mut Vec<Frame>) {
    match tree {
        Tree::Frames(frames) => out.extend(frames),
        Tree::Groups(groups) => groups.into_iter().for_each(|g| into_frames(g.tree, out)),
    }
}

// sort -----------------------------------------------------------------------------------

/// Orders the frames of every innermost group, the groups of the innermost
/// level, or the points of every frame, by attributes (for points also by
/// columns). Missing values go last, also with `desc`. The order is
/// stable.
pub fn sort(id: &str, n: &SortNode, tree: Tree) -> Result<Tree> {
    let keys: Vec<(&str, bool)> = n.by.iter().map(|k| (k.key.as_str(), k.desc)).collect();
    let cmp = |a: &[Option<AttrValue>], b: &[Option<AttrValue>]| {
        keys.iter()
            .enumerate()
            .map(|(i, (_, desc))| cmp_missing_last(a[i].as_ref(), b[i].as_ref(), *desc))
            .find(|o| *o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    };
    match n.target {
        SortTarget::Frames => map_frames(tree, &mut |frames| {
            let values: Vec<Vec<Option<AttrValue>>> = frames
                .iter()
                .map(|f| {
                    keys.iter()
                        .map(|(k, _)| f.attrs.get(*k).map(|a| a.value.clone()))
                        .collect()
                })
                .collect();
            Ok(permute(frames, |a, b| cmp(&values[a], &values[b])))
        }),
        SortTarget::Points => map_frames(tree, &mut |mut frames| {
            for f in &mut frames {
                let values: Vec<Vec<Option<AttrValue>>> = (0..f.len())
                    .map(|i| keys.iter().map(|(k, _)| point_value(f, i, k)).collect())
                    .collect();
                let mut order: Vec<usize> = (0..f.len()).collect();
                order.sort_by(|&a, &b| cmp(&values[a], &values[b]));
                f.select_points(&order);
            }
            Ok(frames)
        }),
        SortTarget::Groups => {
            // An empty result has lost its levels; there is nothing to sort.
            if tree.depth() == 0 && !tree.frames().is_empty() {
                return Err(Problem::setting(
                    id,
                    "target",
                    "the input has no groups to sort",
                ));
            }
            Ok(sort_groups(tree, &mut |groups| {
                let values: Vec<Vec<Option<AttrValue>>> = groups
                    .iter()
                    .map(|g| keys.iter().map(|(k, _)| group_value(g, k)).collect())
                    .collect();
                permute(groups, |a, b| cmp(&values[a], &values[b]))
            }))
        }
    }
}

/// The value of a column or an attribute at point `i`.
fn point_value(f: &Frame, i: usize, name: &str) -> Option<AttrValue> {
    match f.column(name) {
        Some(c) => f.values[c][i].and_then(AttrValue::number),
        None => f.point_attr(i, name).map(|a| a.value.clone()),
    }
}

/// The value of an attribute for a group: its key value, or else the value
/// that all its frames share.
fn group_value(g: &Group, name: &str) -> Option<AttrValue> {
    if let Some((_, v)) = g.key.iter().find(|(n, _)| n == name) {
        return v.as_ref().map(|a| a.value.clone());
    }
    common_attributes(g.tree.frames())
        .remove(name)
        .map(|a| a.value)
}

/// Applies `f` to the groups of the innermost level, in each parent.
fn sort_groups(tree: Tree, f: &mut impl FnMut(Vec<Group>) -> Vec<Group>) -> Tree {
    match tree {
        Tree::Groups(groups) if groups.iter().all(|g| matches!(g.tree, Tree::Frames(_))) => {
            Tree::Groups(f(groups))
        }
        Tree::Groups(groups) => Tree::Groups(
            groups
                .into_iter()
                .map(|g| Group {
                    key: g.key,
                    tree: sort_groups(g.tree, f),
                })
                .collect(),
        ),
        frames => frames,
    }
}

/// `items` in the stable order of `cmp` over their indices.
fn permute<T>(items: Vec<T>, cmp: impl Fn(usize, usize) -> Ordering) -> Vec<T> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| cmp(a, b));
    let mut items: Vec<Option<T>> = items.into_iter().map(Some).collect();
    order
        .into_iter()
        .map(|i| items[i].take().expect("each item once"))
        .collect()
}

// select_latest --------------------------------------------------------------------------

/// Keeps, in every innermost group, the frames of the latest N runs; or,
/// with `target: points`, in every frame the points of the latest N runs.
/// Runs are ordered as the run list orders them: by measured start, then
/// by id. Frames, or points, without a run are dropped.
pub fn select_latest(id: &str, n: &SelectLatestNode, tree: Tree) -> Result<Tree> {
    let runs = match &n.runs {
        Some(s) => *bound(id, "runs", s)? as usize,
        None => 1,
    };
    let tree = match n.target {
        On::Frames => map_frames(tree, &mut |mut frames| {
            let keep = latest(
                frames.iter().map(|f| run_of(|name| f.attrs.get(name))),
                runs,
            );
            frames.retain(|f| {
                run_of(|name| f.attrs.get(name)).is_some_and(|(id, _)| among(&keep, id))
            });
            Ok(frames)
        })?,
        On::Points => map_frames(tree, &mut |mut frames| {
            for f in &mut frames {
                let keep = latest(
                    (0..f.len()).map(|i| run_of(|name| f.point_attr(i, name))),
                    runs,
                );
                let points: Vec<usize> = (0..f.len())
                    .filter(|&i| {
                        run_of(|name| f.point_attr(i, name)).is_some_and(|(id, _)| among(&keep, id))
                    })
                    .collect();
                if points.len() < f.len() {
                    f.select_points(&points);
                }
            }
            Ok(frames)
        })?,
    };
    Ok(prune(tree, n.target == On::Points))
}

/// The run of a frame or point: `run.id` and `run.started_at`.
fn run_of<'a>(
    get: impl Fn(&str) -> Option<&'a crate::chart::frame::Attr>,
) -> Option<(&'a AttrValue, Option<&'a AttrValue>)> {
    let id = &get("run.id")?.value;
    Some((id, get("run.started_at").map(|a| &a.value)))
}

/// The ids of the latest `n` runs among `runs`, by [`run_key`].
fn latest<'a>(
    runs: impl Iterator<Item = Option<(&'a AttrValue, Option<&'a AttrValue>)>>,
    n: usize,
) -> HashSet<String> {
    let mut seen: HashMap<String, (&AttrValue, Option<&AttrValue>)> = HashMap::new();
    for (id, started) in runs.flatten() {
        seen.entry(run_key(id)).or_insert((id, started));
    }
    let mut runs: Vec<(String, (&AttrValue, Option<&AttrValue>))> = seen.into_iter().collect();
    // Newest first; a run without a start counts as the oldest.
    runs.sort_by(|(_, a), (_, b)| {
        cmp_missing_last(a.1, b.1, true).then_with(|| b.0.total_cmp(a.0))
    });
    runs.into_iter().take(n).map(|(key, _)| key).collect()
}

/// A run id as a key for hashing: run ids are texts.
fn run_key(id: &AttrValue) -> String {
    match id {
        AttrValue::Text(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn among(ids: &HashSet<String>, id: &AttrValue) -> bool {
    match id {
        AttrValue::Text(s) => ids.contains(s),
        other => ids.contains(&run_key(other)),
    }
}

// concatenate ----------------------------------------------------------------------------

/// The frames of each innermost group become one frame with all their
/// points, so that level disappears; a flat list becomes one frame. The
/// frames need the same columns. The new frame keeps the attributes they
/// share, and every point the attributes of the frame it came from.
pub fn concatenate(id: &str, tree: Tree) -> Result<Tree> {
    merge_innermost(tree, &mut |frames| concat(id, frames))
}

fn concat(id: &str, frames: Vec<Frame>) -> Result<Option<Frame>> {
    let Some(first) = frames.first() else {
        return Ok(None);
    };
    let columns = first.columns.clone();
    if let Some(other) = frames.iter().find(|f| f.columns != columns) {
        return Err(Problem::node(
            id,
            format!(
                "concatenate needs frames with the same columns, but one has {} and another {}",
                show_columns(&columns),
                show_columns(&other.columns)
            ),
        ));
    }
    let common = common_attributes(&frames);
    let total: usize = frames.iter().map(Frame::len).sum();
    let mut values: Vec<Vec<Option<f64>>> = vec![Vec::with_capacity(total); columns.len()];
    let mut points: Vec<Attributes> = Vec::with_capacity(total);
    let mut seen_names: HashSet<String> = HashSet::new();
    for f in frames {
        let own: Attributes = f
            .attrs
            .into_iter()
            .filter(|(name, _)| !common.contains_key(name))
            .collect();
        seen_names.extend(own.keys().cloned());
        let n = f.values.first().map_or(0, Vec::len);
        let mut point_attrs = f.point_attrs.map(Vec::into_iter);
        for _ in 0..n {
            let mut attrs = own.clone();
            if let Some(p) = point_attrs.as_mut().and_then(Iterator::next) {
                seen_names.extend(p.keys().cloned());
                attrs.extend(p);
            }
            points.push(attrs);
        }
        for (c, column) in f.values.into_iter().enumerate() {
            values[c].extend(column);
        }
    }
    let mut frame = Frame::new(common, columns, values);
    if !seen_names.is_empty() {
        frame.point_attrs = Some(points);
    }
    Ok(Some(frame))
}

fn show_columns(columns: &[Column]) -> String {
    if columns.is_empty() {
        return "no columns".into();
    }
    let shown: Vec<String> = columns
        .iter()
        .map(|c| {
            let mut shown = c.name.clone();
            if !c.dimension.is_empty() {
                shown.push_str(&format!(" ({})", c.dimension));
            }
            if !c.unit.is_empty() {
                shown.push_str(&format!(" in {}", c.unit));
            }
            shown
        })
        .collect();
    shown.join(", ")
}
