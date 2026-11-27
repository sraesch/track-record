//! Evaluation of the nodes of a chart (ADR 0022, "Nodes", "Evaluation").
//!
//! Every node is a pure function from the tree of its input to a new tree.
//! Only sources read data; the server loads them and hands them to [`run`]
//! by node id. [`run`] evaluates the nodes in the order of their inputs,
//! each once, and drops a result as soon as no other node needs it.
//!
//! Errors are [`Problem`]s of the node, like the problems of validation:
//! a run of a valid spec fails only where the data does not fit, e.g.
//! frames with different columns in a concatenate.

pub mod compute;
pub mod derive;
pub mod structure;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};

use super::frame::{Column, Frame, Group, Tree};
use super::graph::Graph;
use super::problem::Problem;
use super::spec::{FlattenLevels, NodeKind, Spec};
use super::variables::Setting;
use crate::units::UnitCatalogue;

pub type Result<T> = std::result::Result<T, Problem>;

/// Evaluates a bound spec ([`Spec::bind`]) up to its visualization, or up
/// to the node `until` for a preview. `sources` holds the output of every
/// source that the evaluated nodes read, by node id. Returns the tree that
/// reaches the visualization, or the output of `until`.
///
/// `max_points` is the budget of points the run may hold at once (ADR
/// 0022, "Limits"): the sources and every result that a node still needs.
/// Nodes that make points (resample, and a match that repeats a partner)
/// check it before they allocate; a run over it fails at that node.
pub fn run(
    spec: &Spec,
    graph: &Graph,
    mut sources: BTreeMap<String, Tree>,
    until: Option<&str>,
    catalogue: &UnitCatalogue,
    max_points: usize,
) -> Result<Tree> {
    let target = match until {
        Some(id) => graph
            .index(id)
            .ok_or_else(|| Problem::spec_setting("until", format!("there is no node {id:?}")))?,
        None => graph.visualization(),
    };
    let needed = graph.upstream(target);
    // How many of the evaluated nodes still read each result.
    let mut readers = vec![0usize; spec.nodes.len()];
    for &i in &needed {
        for (_, j) in graph.inputs(i) {
            readers[*j] += 1;
        }
    }
    let mut results: Vec<Option<Tree>> = vec![None; spec.nodes.len()];
    // Points held by the sources and the results still needed.
    let mut held: usize = sources.values().map(points).sum();
    // The columns of the inputs of each match when it ran, by node.
    let mut match_columns: HashMap<usize, BTreeMap<String, Vec<Column>>> = HashMap::new();
    for &i in &needed {
        let node = &spec.nodes[i];
        let mut inputs = Vec::new();
        // Points of the inputs this node consumes, its own copies
        // included: freed by it.
        let mut freed = 0;
        for (name, j) in graph.inputs(i) {
            readers[*j] -= 1;
            let tree = if readers[*j] == 0 {
                results[*j].take()
            } else {
                // A copy for this reader: held as well until it is used.
                let copy = results[*j].clone();
                held = held.saturating_add(copy.as_ref().map_or(0, points));
                copy
            };
            let tree = tree.expect("inputs are evaluated before their readers");
            freed += points(&tree);
            inputs.push((name.clone(), tree));
        }
        if held > max_points {
            return Err(over_budget(&node.id, max_points));
        }
        held -= freed;
        let limit = max_points.saturating_sub(held);
        if node.kind.is_match() {
            match_columns.insert(
                i,
                inputs
                    .iter()
                    .filter_map(|(name, t)| {
                        let first = t.frames().into_iter().next()?;
                        Some((
                            name.clone().unwrap_or_else(|| "a".into()),
                            first.columns.clone(),
                        ))
                    })
                    .collect(),
            );
        }
        let out = match &node.kind {
            NodeKind::Source(_) => {
                let tree = sources.remove(&node.id).ok_or_else(|| {
                    Problem::node(&node.id, "the data of this source was not loaded")
                })?;
                // Counted when it was loaded.
                held -= points(&tree);
                tree
            }
            NodeKind::Combine(n) => {
                let tree = inputs
                    .into_iter()
                    .next()
                    .map(|(_, t)| t)
                    .unwrap_or_default();
                let found = match_inputs(spec, graph, i, &match_columns);
                compute::combine(&node.id, n, tree, found.as_ref(), catalogue)?
            }
            kind => evaluate(&node.id, kind, inputs, catalogue, limit)?,
        };
        debug_assert!(out.check().is_ok(), "{}: {:?}", node.id, out.check());
        held = held.saturating_add(points(&out));
        if held > max_points {
            return Err(over_budget(&node.id, max_points));
        }
        results[i] = Some(out);
    }
    Ok(results[target].take().expect("the target was evaluated"))
}

/// The number of points of all frames of a tree.
fn points(tree: &Tree) -> usize {
    tree.frames().iter().map(|f| f.len()).sum()
}

fn over_budget(id: &str, max_points: usize) -> Problem {
    Problem::node(
        id,
        format!(
            "the chart would hold more than {max_points} points here; narrow the filter of a \
             source, or reduce the frames earlier"
        ),
    )
}

/// The inputs of the match whose level a combine at node `i` merges: the
/// match it reads through nodes that keep that level innermost, or that
/// add a level below it and merge it again (a group, then a concatenate).
/// The columns the match saw count only where no node on the way changed
/// them.
fn match_inputs(
    spec: &Spec,
    graph: &Graph,
    i: usize,
    match_columns: &HashMap<usize, BTreeMap<String, Vec<Column>>>,
) -> Option<compute::MatchInputs> {
    let mut j = graph.inputs(i).first()?.1;
    // Levels below the match level at node j.
    let mut below = 0usize;
    let mut same_columns = true;
    loop {
        match &spec.nodes[j].kind {
            NodeKind::Match(_) if below == 0 => {
                let names = graph
                    .inputs(j)
                    .iter()
                    .map(|(name, _)| name.clone().unwrap_or_else(|| "a".into()))
                    .collect();
                let columns = match (same_columns, match_columns.get(&j)) {
                    (true, Some(c)) => c.clone(),
                    _ => BTreeMap::new(),
                };
                return Some(compute::MatchInputs { names, columns });
            }
            NodeKind::Filter(_)
            | NodeKind::Derive(_)
            | NodeKind::Sort(_)
            | NodeKind::SelectLatest(_)
            | NodeKind::Resample(_) => {}
            NodeKind::Group(_) if below > 0 => below -= 1,
            NodeKind::Concatenate(_) => below += 1,
            NodeKind::Reduce(_) => {
                below += 1;
                same_columns = false;
            }
            NodeKind::Combine(n) if n.functions.is_some() => {
                below += 1;
                same_columns = false;
            }
            NodeKind::Flatten(n) => match n.levels {
                None => below += 1,
                Some(FlattenLevels::Count(k)) => below += k as usize,
                Some(FlattenLevels::All) => return None,
            },
            _ => return None,
        }
        j = graph.inputs(j).first()?.1;
    }
}

/// Evaluates one node other than a source or a combine on its inputs.
/// Inputs come with the names of a match's inputs, `a` first.
fn evaluate(
    id: &str,
    kind: &NodeKind,
    inputs: Vec<(Option<String>, Tree)>,
    catalogue: &UnitCatalogue,
    limit: usize,
) -> Result<Tree> {
    if let NodeKind::Match(n) = kind {
        let named = inputs
            .into_iter()
            .map(|(name, t)| (name.unwrap_or_else(|| "a".into()), t))
            .collect();
        return compute::match_frames(id, n, named, limit);
    }
    let mut inputs = inputs.into_iter().map(|(_, t)| t);
    let input = inputs
        .next()
        .ok_or_else(|| Problem::node(id, "the node has no input"))?;
    match kind {
        NodeKind::Source(_) => unreachable!("sources are loaded by the server"),
        NodeKind::Filter(n) => structure::filter(id, n, input),
        NodeKind::Derive(n) => derive::derive(id, n, input, catalogue),
        NodeKind::Group(n) => structure::group(id, n, input),
        NodeKind::Flatten(n) => Ok(structure::flatten(n, input)),
        NodeKind::Sort(n) => structure::sort(id, n, input),
        NodeKind::SelectLatest(n) => structure::select_latest(id, n, input),
        NodeKind::Concatenate(_) => structure::concatenate(id, input),
        NodeKind::Resample(n) => compute::resample(id, n, input, catalogue, limit),
        NodeKind::Reduce(n) => compute::reduce(id, n, input, catalogue),
        NodeKind::Combine(n) => compute::combine(id, n, input, None, catalogue),
        NodeKind::Match(_) => unreachable!("matched above"),
        // The visualization draws what reaches it.
        NodeKind::Line(_)
        | NodeKind::Scatter(_)
        | NodeKind::Bar(_)
        | NodeKind::Box(_)
        | NodeKind::Heatmap(_)
        | NodeKind::Value(_)
        | NodeKind::Table(_) => Ok(input),
    }
}

/// The value of a bound setting. A variable left in a spec that was not
/// bound is an error of the node.
pub(crate) fn bound<'a, T>(id: &str, setting: &str, s: &'a Setting<T>) -> Result<&'a T> {
    match s {
        Setting::Value(v) => Ok(v),
        Setting::Var(name) => Err(Problem::setting(
            id,
            setting,
            format!("variable {name:?} has no value"),
        )),
    }
}

/// Replaces every innermost list of frames (the root list of a flat tree)
/// by the tree `f` makes of it. Groups keep their keys and order.
pub(crate) fn map_innermost(
    tree: Tree,
    f: &mut impl FnMut(Vec<Frame>) -> Result<Tree>,
) -> Result<Tree> {
    match tree {
        Tree::Frames(frames) => f(frames),
        Tree::Groups(groups) => Ok(Tree::Groups(
            groups
                .into_iter()
                .map(|g| {
                    Ok(Group {
                        key: g.key,
                        tree: map_innermost(g.tree, f)?,
                    })
                })
                .collect::<Result<_>>()?,
        )),
    }
}

/// Replaces every innermost list of frames by the list `f` makes of it.
pub(crate) fn map_frames(
    tree: Tree,
    f: &mut impl FnMut(Vec<Frame>) -> Result<Vec<Frame>>,
) -> Result<Tree> {
    map_innermost(tree, &mut |frames| f(frames).map(Tree::Frames))
}

/// Replaces every innermost group by the frame `merge` makes of its
/// frames, so that level disappears; a flat list becomes one frame. Used by
/// the nodes that merge frames: concatenate, reduce and combine. `merge`
/// returns `None` for a group that gives no frame.
pub(crate) fn merge_innermost(
    tree: Tree,
    merge: &mut impl FnMut(Vec<Frame>) -> Result<Option<Frame>>,
) -> Result<Tree> {
    match tree {
        Tree::Frames(frames) => Ok(Tree::Frames(merge(frames)?.into_iter().collect())),
        Tree::Groups(groups) if groups.iter().all(|g| matches!(g.tree, Tree::Frames(_))) => {
            let mut frames = Vec::with_capacity(groups.len());
            for g in groups {
                let Tree::Frames(members) = g.tree else {
                    unreachable!("checked above")
                };
                frames.extend(merge(members)?);
            }
            Ok(Tree::Frames(frames))
        }
        Tree::Groups(groups) => Ok(Tree::Groups(
            groups
                .into_iter()
                .map(|g| {
                    Ok(Group {
                        key: g.key,
                        tree: merge_innermost(g.tree, merge)?,
                    })
                })
                .collect::<Result<_>>()?,
        )),
    }
}

/// Removes groups without frames at every level, and with `points` also
/// the frames without points, which a node that keeps points left empty.
/// The root stays, empty or not.
pub(crate) fn prune(tree: Tree, points: bool) -> Tree {
    match tree {
        Tree::Frames(mut frames) => {
            if points {
                frames.retain(|f| !f.is_empty());
            }
            Tree::Frames(frames)
        }
        Tree::Groups(groups) => Tree::Groups(
            groups
                .into_iter()
                .map(|g| Group {
                    key: g.key,
                    tree: prune(g.tree, points),
                })
                .filter(|g| !is_empty(&g.tree))
                .collect(),
        ),
    }
}

fn is_empty(tree: &Tree) -> bool {
    match tree {
        Tree::Frames(frames) => frames.is_empty(),
        Tree::Groups(groups) => groups.is_empty(),
    }
}
