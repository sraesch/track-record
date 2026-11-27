//! The nodes that compute values (ADR 0022, "Nodes"): resample, match,
//! reduce and combine.
//!
//! Statistics come from [`crate::stats`], grids and interpolation from
//! [`crate::series`], and the expressions of a combine use the DSL's
//! grammar, scalar functions and unit rules, as a derive does. No value
//! stays no value (FR-12a): it is counted, never computed with.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use super::derive::grown;
use super::structure::cmp_attrs;
use super::{Result, bound, map_frames, map_innermost, merge_innermost, prune};
use crate::chart::attributes::{self, Family, MatchKey};
use crate::chart::frame::{
    Attr, AttrValue, Attributes, Column, Frame, Group, Tree, common_attributes,
};
use crate::chart::problem::Problem;
use crate::chart::spec::{
    CombineNode, DEFAULT_CONFIDENCE, DEFAULT_GRID_POINTS, Expression, Function, FunctionName,
    MatchNode, ReduceNode, ResampleNode, Unmatched,
};
use crate::dsl::check::{Checked, Env, SeriesUnits, Signature, check};
use crate::dsl::eval::{EvalError, Inputs, Value, eval};
use crate::dsl::{Diagnostic, Expr, parse_columns};
use crate::series::{self, Series};
use crate::stats::{self, Summary};
use crate::units::UnitCatalogue;

// Functions ------------------------------------------------------------------------------

/// Columns that count: never the default column of a reduce or combine.
fn is_count(name: &str) -> bool {
    matches!(name, "n" | "n_no_value" | "count")
}

fn column_names(f: &Frame) -> String {
    if f.columns.is_empty() {
        return "none".into();
    }
    let names: Vec<&str> = f.columns.iter().map(|c| c.name.as_str()).collect();
    names.join(", ")
}

/// The index of the column of `f` that a reduce or combine with functions
/// reads: the one named, or the only one that is not `x` or a count.
fn value_column(id: &str, f: &Frame, column: Option<&str>) -> Result<usize> {
    if let Some(name) = column {
        return f.column(name).ok_or_else(|| {
            Problem::setting(
                id,
                "column",
                format!(
                    "there is no column {name:?}; the columns are {}",
                    column_names(f)
                ),
            )
        });
    }
    let candidates: Vec<usize> = (0..f.columns.len())
        .filter(|&i| f.columns[i].name != "x" && !is_count(&f.columns[i].name))
        .collect();
    match candidates.as_slice() {
        [one] => Ok(*one),
        [] => Err(Problem::setting(
            id,
            "column",
            "the frames have no column to compute with",
        )),
        _ => Err(Problem::setting(
            id,
            "column",
            format!(
                "the frames have the columns {}: name one in column",
                column_names(f)
            ),
        )),
    }
}

/// The percentiles the functions ask for.
fn percentiles(functions: &[Function]) -> Vec<f64> {
    functions
        .iter()
        .filter(|f| f.name == FunctionName::Percentile)
        .filter_map(|f| f.p)
        .collect()
}

/// The value of a function of `values`, whose summary is `s`. A function
/// without a result is no value, never an error.
fn apply(f: &Function, s: &Summary, values: &[Option<f64>]) -> Option<f64> {
    match f.name {
        FunctionName::Mean => s.mean,
        FunctionName::Median => s.median,
        FunctionName::Min => s.min,
        FunctionName::Max => s.max,
        FunctionName::Sum => stats::sum(values),
        FunctionName::Stddev => s.stddev,
        FunctionName::Percentile => {
            let p = f.p?;
            s.percentiles.iter().find(|(q, _)| *q == p)?.1
        }
        FunctionName::CiLow => s.ci.map(|c| c.0),
        FunctionName::CiHigh => s.ci.map(|c| c.1),
        FunctionName::Geomean => stats::geomean(values),
        FunctionName::First => stats::first(values),
        FunctionName::Last => stats::last(values),
        FunctionName::Count => Some(s.n as f64),
    }
}

/// What the output columns of functions compute.
enum Out<'a> {
    Function(&'a Function),
    N,
    NoValue,
}

/// The columns of `functions` over a column in `unit`, in their order:
/// counts have no unit, the others `unit`. `n` and `n_no_value` follow
/// unless a function gives them: every aggregate states how many values it
/// had and how many had none (FR-12a).
fn function_columns<'a>(functions: &'a [Function], unit: &str) -> Vec<(Column, Out<'a>)> {
    let mut out: Vec<(Column, Out)> = Vec::new();
    for f in functions {
        let name = f.column();
        if out.iter().any(|(c, _)| c.name == name) {
            continue; // reported by validation
        }
        let unit = if f.is_count() { "" } else { unit };
        out.push((Column::new(name, unit), Out::Function(f)));
    }
    for (name, what) in [("n", Out::N), ("n_no_value", Out::NoValue)] {
        if !out.iter().any(|(c, _)| c.name == name) {
            out.push((Column::new(name, ""), what));
        }
    }
    out
}

/// The value of an output column. A result that is not finite (a sum
/// beyond the range of `f64`) is no value, never NaN in a frame.
fn compute(what: &Out, s: &Summary, values: &[Option<f64>]) -> Option<f64> {
    let v = match what {
        Out::Function(f) => apply(f, s, values),
        Out::N => Some(s.n as f64),
        Out::NoValue => Some(s.n_no_value as f64),
    };
    v.filter(|v| v.is_finite())
}

/// The input all frames come from, if they share one: a merge inside a
/// match group keeps the frame's place in the match.
fn common_input(frames: &[Frame]) -> Option<String> {
    let first = frames.first()?.input.as_ref()?;
    frames
        .iter()
        .all(|f| f.input.as_ref() == Some(first))
        .then(|| first.clone())
}

/// The error of a node whose output would not fit into what is left of
/// the chart's budget of points.
fn over_budget(id: &str, limit: usize) -> Problem {
    Problem::node(
        id,
        format!(
            "the output would hold more than the {limit} points left of the chart's budget; \
             narrow the filter of a source, use fewer grid points, or reduce the frames first"
        ),
    )
}

/// The factor that converts the values of a column in `from` to `to`.
fn factor(id: &str, catalogue: &UnitCatalogue, column: &str, from: &str, to: &str) -> Result<f64> {
    if from == to {
        return Ok(1.0);
    }
    catalogue.conversion_factor(from, to).ok_or_else(|| {
        Problem::node(
            id,
            format!(
                "the frames have {column} in {from:?} and in {to:?}, which cannot be converted"
            ),
        )
    })
}

fn scaled(values: &[Option<f64>], k: f64) -> Vec<Option<f64>> {
    if k == 1.0 {
        return values.to_vec();
    }
    values.iter().map(|v| v.map(|x| x * k)).collect()
}

// reduce ---------------------------------------------------------------------------------

/// Turns every frame of each innermost group into one point of a new
/// frame, so that level disappears; a flat list becomes one frame. The
/// point holds the functions of the frame's column and its counts, and the
/// attributes of the frame that the frames do not share.
pub fn reduce(id: &str, n: &ReduceNode, tree: Tree, catalogue: &UnitCatalogue) -> Result<Tree> {
    let column = match &n.column {
        Some(s) => Some(bound(id, "column", s)?.as_str()),
        None => None,
    };
    let confidence = match &n.confidence {
        Some(s) => *bound(id, "confidence", s)?,
        None => DEFAULT_CONFIDENCE,
    };
    let ps = percentiles(&n.functions);
    merge_innermost(tree, &mut |frames| {
        if frames.is_empty() {
            return Ok(None);
        }
        let common = common_attributes(&frames);
        let mut unit: Option<(String, String)> = None;
        let mut outputs: Option<Vec<(Column, Out)>> = None;
        let mut values: Vec<Vec<Option<f64>>> = Vec::new();
        let mut points: Vec<Attributes> = Vec::with_capacity(frames.len());
        for f in &frames {
            let c = value_column(id, f, column)?;
            let col = &f.columns[c];
            let k = match &unit {
                None => {
                    unit = Some((col.name.clone(), col.unit.clone()));
                    1.0
                }
                Some((name, to)) => factor(id, catalogue, name, &col.unit, to)?,
            };
            let outputs = outputs.get_or_insert_with(|| {
                let o = function_columns(&n.functions, &col.unit);
                values = vec![Vec::with_capacity(frames.len()); o.len()];
                o
            });
            let column = scaled(&f.values[c], k);
            let s = stats::summarize(&column, &ps, confidence);
            for ((_, what), out) in outputs.iter().zip(&mut values) {
                out.push(compute(what, &s, &column));
            }
            points.push(
                f.attrs
                    .iter()
                    .filter(|(name, _)| !common.contains_key(*name))
                    .map(|(name, a)| (name.clone(), a.clone()))
                    .collect(),
            );
        }
        let columns = outputs
            .expect("a group with frames")
            .into_iter()
            .map(|(c, _)| c)
            .collect();
        let mut frame = Frame::new(common, columns, values);
        if points.iter().any(|p| !p.is_empty()) {
            frame.point_attrs = Some(points);
        }
        frame.input = common_input(&frames);
        Ok(Some(frame))
    })
}

// resample -------------------------------------------------------------------------------

/// Puts the frames of each innermost group onto one grid of x: `points`
/// evenly spaced values from `from` to `to`, by default the range of x of
/// the group's frames. Every other column is interpolated at the grid
/// (FR-19); before a series, next to no value, and after it unless step
/// interpolation holds its last value, there is no value. x is converted
/// to the unit of the first frame of the group. The output may hold at most
/// `limit` points.
pub fn resample(
    id: &str,
    n: &ResampleNode,
    tree: Tree,
    catalogue: &UnitCatalogue,
    limit: usize,
) -> Result<Tree> {
    let points = match &n.points {
        Some(s) => *bound(id, "points", s)? as usize,
        None => DEFAULT_GRID_POINTS as usize,
    };
    let from = match &n.from {
        Some(s) => Some(*bound(id, "from", s)?),
        None => None,
    };
    let to = match &n.to {
        Some(s) => Some(*bound(id, "to", s)?),
        None => None,
    };
    let mut total = 0usize;
    map_frames(tree, &mut |mut frames| {
        // x of each frame in the unit of the first, and its points that
        // have an x, ordered by x.
        let mut xs: Vec<(usize, Vec<f64>, Vec<usize>)> = Vec::with_capacity(frames.len());
        let mut extent: Option<(f64, f64)> = None;
        let mut unit: Option<Column> = None;
        for f in &frames {
            let Some(xc) = f.column("x") else {
                return Err(Problem::node(
                    id,
                    "resample needs frames with an x column: the points of a series",
                ));
            };
            let col = &f.columns[xc];
            let k = match &unit {
                None => {
                    unit = Some(col.clone());
                    1.0
                }
                Some(first) if first.dimension != col.dimension => {
                    return Err(Problem::node(
                        id,
                        format!(
                            "the frames have x of {:?} and of {:?}, which are not one axis",
                            first.dimension, col.dimension
                        ),
                    ));
                }
                Some(first) => catalogue
                    .conversion_factor(&col.unit, &first.unit)
                    .ok_or_else(|| {
                        Problem::node(
                            id,
                            format!(
                                "the frames have x in {:?} and in {:?}, which cannot be \
                                 converted onto one grid",
                                first.unit, col.unit
                            ),
                        )
                    })?,
            };
            let x: Vec<f64> = f.values[xc]
                .iter()
                .map(|v| v.map_or(f64::NAN, |v| v * k))
                .collect();
            let mut order: Vec<usize> = (0..f.len()).filter(|&i| x[i].is_finite()).collect();
            order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
            if let (Some(&lo), Some(&hi)) = (order.first(), order.last()) {
                let (lo, hi) = (x[lo], x[hi]);
                extent = Some(extent.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
            }
            xs.push((xc, x, order));
        }
        let grid = match (from.or(extent.map(|e| e.0)), to.or(extent.map(|e| e.1))) {
            (Some(lo), Some(hi)) if lo <= hi => series::grid(lo, hi, points),
            _ => Vec::new(),
        };
        total = total.saturating_add(frames.len().saturating_mul(grid.len()));
        if total > limit {
            return Err(over_budget(id, limit));
        }
        for (f, (xc, all, order)) in frames.iter_mut().zip(xs) {
            let x: Vec<f64> = order.iter().map(|&i| all[i]).collect();
            let values = f
                .values
                .iter()
                .enumerate()
                .map(|(c, column)| {
                    if c == xc {
                        return grid.iter().map(|g| Some(*g)).collect();
                    }
                    let y = order
                        .iter()
                        .map(|&i| column[i].unwrap_or(f64::NAN))
                        .collect();
                    Series::new(x.clone(), y).resample(&grid, n.interp)
                })
                .collect();
            f.values = values;
            if let Some(first) = &unit {
                f.columns[xc] = first.clone();
            }
            // The points are new.
            f.point_attrs = None;
        }
        Ok(frames)
    })
}

// match ----------------------------------------------------------------------------------

/// The key of a frame in a match: its attributes that the key covers, by
/// name.
type MatchKeyValues = Vec<(String, Attr)>;

fn key_of(key: &MatchKey, f: &Frame) -> MatchKeyValues {
    f.attrs
        .iter()
        .filter(|(name, _)| key.includes(name))
        .map(|(name, a)| (name.clone(), a.clone()))
        .collect()
}

/// Orders keys as a group orders values: two keys are equal if they hold
/// the same names with the same values and units.
fn cmp_key(a: &MatchKeyValues, b: &MatchKeyValues) -> Ordering {
    for ((n, x), (m, y)) in a.iter().zip(b) {
        let o = n.cmp(m).then_with(|| cmp_attrs(Some(x), Some(y)));
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

fn show_value(v: &AttrValue) -> String {
    match v {
        AttrValue::Bool(b) => b.to_string(),
        AttrValue::Number(n) => n.to_string(),
        AttrValue::Text(s) => format!("{s:?}"),
        AttrValue::Time(t) => t.to_rfc3339(),
        AttrValue::List(items) => {
            let items: Vec<String> = items.iter().map(show_value).collect();
            format!("[{}]", items.join(", "))
        }
    }
}

fn show_key(key: &MatchKeyValues) -> String {
    const MAX: usize = 8;
    if key.is_empty() {
        return "the empty key".into();
    }
    let mut shown: Vec<String> = key
        .iter()
        .take(MAX)
        .map(|(name, a)| {
            let unit = if a.unit.is_empty() {
                String::new()
            } else {
                format!(" {}", a.unit)
            };
            format!("{name} = {}{unit}", show_value(&a.value))
        })
        .collect();
    if key.len() > MAX {
        shown.push(format!("and {} more", key.len() - MAX));
    }
    shown.join(", ")
}

/// Pairs each frame of input `a` with the frames of the other inputs that
/// have the same key: each becomes a group of `a`'s frame and its
/// partners, where `a`'s tree had the frame. A frame of `a` without a
/// partner in every other input is dropped, unless `unmatched` keeps it.
/// `inputs` holds the trees by input name, `a` first. The output may hold
/// at most `limit` points; a partner of several frames counts for each.
pub fn match_frames(
    id: &str,
    n: &MatchNode,
    inputs: Vec<(String, Tree)>,
    limit: usize,
) -> Result<Tree> {
    let except = match &n.except {
        Some(s) => Some(bound(id, "except", s)?.as_slice()),
        None => None,
    };
    let only = match &n.only {
        Some(s) => Some(bound(id, "only", s)?.as_slice()),
        None => None,
    };
    let key = match (only, except) {
        (Some(only), _) => MatchKey::Only(only),
        (None, except) => MatchKey::Except(except.unwrap_or(&[])),
    };
    let mut inputs = inputs.into_iter();
    let (a, tree) = inputs
        .next()
        .ok_or_else(|| Problem::node(id, "the node has no input"))?;
    // The frames of each other input, ordered by key.
    let mut others: Vec<Vec<(MatchKeyValues, Frame)>> = Vec::new();
    for (name, tree) in inputs {
        let mut frames: Vec<(MatchKeyValues, Frame)> = Vec::new();
        into_frames(tree, &mut |mut f| {
            f.input = Some(name.clone());
            frames.push((key_of(&key, &f), f));
        });
        frames.sort_by(|x, y| cmp_key(&x.0, &y.0));
        if let Some(w) = frames
            .windows(2)
            .find(|w| cmp_key(&w[0].0, &w[1].0) == Ordering::Equal)
        {
            return Err(Problem::node(
                id,
                format!(
                    "input {name} has two frames with the key {}: a frame of a can have only \
                     one partner per input; group and merge them first, or change the key",
                    show_key(&w[0].0)
                ),
            ));
        }
        others.push(frames);
    }
    let mut total = 0usize;
    let tree = map_innermost(tree, &mut |frames| {
        let mut groups = Vec::with_capacity(frames.len());
        for mut f in frames {
            let k = key_of(&key, &f);
            let found: Vec<Option<usize>> = others
                .iter()
                .map(|o| o.binary_search_by(|(ok, _)| cmp_key(ok, &k)).ok())
                .collect();
            let complete = found.iter().all(Option::is_some);
            if !complete && n.unmatched == Unmatched::Drop {
                continue;
            }
            let points = found
                .iter()
                .zip(&others)
                .filter_map(|(i, o)| Some(o[(*i)?].1.len()))
                .fold(f.len(), usize::saturating_add);
            total = total.saturating_add(points);
            if total > limit {
                return Err(over_budget(id, limit));
            }
            let mut members = Vec::with_capacity(1 + others.len());
            for (i, o) in found.iter().zip(&others) {
                if let Some(i) = i {
                    members.push(o[*i].1.clone());
                }
            }
            f.input = Some(a.clone());
            members.insert(0, f);
            groups.push(Group {
                key: group_key(&key, k),
                tree: Tree::Frames(members),
            });
        }
        Ok(Tree::Groups(groups))
    })?;
    Ok(prune(tree, false))
}

/// The key of a match group: the key of `a`'s frame, and the attributes
/// `only` names that it has no value for, as a group key has them.
fn group_key(key: &MatchKey, values: MatchKeyValues) -> Vec<(String, Option<Attr>)> {
    let mut out: Vec<(String, Option<Attr>)> = values
        .into_iter()
        .map(|(name, v)| (name, Some(v)))
        .collect();
    if let MatchKey::Only(only) = key {
        for name in only.iter() {
            if Family::from_key(name).is_none()
                && key.includes(name)
                && !out.iter().any(|(n, _)| n == name)
            {
                out.push((name.clone(), None));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
    }
    out
}

fn into_frames(tree: Tree, f: &mut impl FnMut(Frame)) {
    match tree {
        Tree::Frames(frames) => frames.into_iter().for_each(f),
        Tree::Groups(groups) => groups.into_iter().for_each(|g| into_frames(g.tree, f)),
    }
}

// combine: alignment ---------------------------------------------------------------------

/// How a frame gives its value at a point of the combined frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    /// Point by point.
    Points,
    /// One point and no x: the same value at every point.
    Scalar,
    /// No points: no value at every point.
    Empty,
}

/// How the frames of a group line up point by point.
struct Aligned {
    len: usize,
    /// `x` of the combined frame, in the unit of the first frame with one.
    x: Option<(Column, Vec<Option<f64>>)>,
    reads: Vec<Read>,
}

impl Aligned {
    /// The value of column `c` of frame `k` at point `i`.
    fn value(&self, frames: &[Frame], k: usize, c: usize, i: usize) -> Option<f64> {
        match self.reads[k] {
            Read::Points => frames[k].values[c][i],
            Read::Scalar => frames[k].values[c][0],
            Read::Empty => None,
        }
    }

    /// The point of frame `k` that point `i` reads.
    fn point(&self, k: usize, i: usize) -> Option<usize> {
        match self.reads[k] {
            Read::Points => Some(i),
            Read::Scalar => Some(0),
            Read::Empty => None,
        }
    }
}

fn describe(frames: &[Frame], k: usize) -> String {
    match &frames[k].input {
        Some(name) => format!("the frame of {name}"),
        None => format!("frame {}", k + 1),
    }
}

/// Lines up the frames of a group: frames with x need the same x values
/// (after converting x to the unit of the first), frames without x the
/// same number of points. A frame with one point and no x applies to every
/// point; a frame without points is no value at every point. Resampling is
/// never implicit.
fn align(id: &str, frames: &[Frame], catalogue: &UnitCatalogue) -> Result<Aligned> {
    // One axis for all frames with x, also those without points, whose x
    // the result may keep.
    let mut axis: Option<(usize, &Column)> = None;
    for (k, f) in frames.iter().enumerate() {
        let Some(c) = f.column("x") else { continue };
        let col = &f.columns[c];
        let Some((j, first)) = axis else {
            axis = Some((k, col));
            continue;
        };
        if first.dimension != col.dimension {
            return Err(Problem::node(
                id,
                format!(
                    "x of {} is {:?} and x of {} {:?}: they are not one axis",
                    describe(frames, j),
                    first.dimension,
                    describe(frames, k),
                    col.dimension
                ),
            ));
        }
        if !catalogue.convertible(&col.unit, &first.unit) {
            return Err(Problem::node(
                id,
                format!(
                    "x of {} is in {:?} and x of {} in {:?}, which cannot be converted",
                    describe(frames, j),
                    first.unit,
                    describe(frames, k),
                    col.unit
                ),
            ));
        }
    }
    let mut reads = Vec::with_capacity(frames.len());
    let mut base: Option<usize> = None;
    let mut len = 0;
    let mut x: Option<(Column, Vec<Option<f64>>)> = None;
    for (k, f) in frames.iter().enumerate() {
        let xc = f.column("x");
        if f.is_empty() {
            reads.push(Read::Empty);
            continue;
        }
        if xc.is_none() && f.len() == 1 {
            reads.push(Read::Scalar);
            continue;
        }
        reads.push(Read::Points);
        let Some(b) = base else {
            base = Some(k);
            len = f.len();
            x = xc.map(|c| (f.columns[c].clone(), f.values[c].clone()));
            continue;
        };
        let apart = |reason: String| {
            Problem::node(
                id,
                format!(
                    "{} and {} do not line up point by point: {reason}; put them on a common \
                     grid with a resample node first",
                    describe(frames, b),
                    describe(frames, k)
                ),
            )
        };
        if f.len() != len {
            return Err(apart(format!(
                "one has {len} points, the other {}",
                f.len()
            )));
        }
        match (&x, xc) {
            (Some((col, xs)), Some(c)) => {
                let k = catalogue
                    .conversion_factor(&f.columns[c].unit, &col.unit)
                    .ok_or_else(|| {
                        Problem::node(
                            id,
                            format!(
                                "x of {} is in {:?} and x of {} in {:?}, which cannot be \
                                 converted",
                                describe(frames, b),
                                col.unit,
                                describe(frames, k),
                                f.columns[c].unit
                            ),
                        )
                    })?;
                // The same unit compares exactly; a conversion may round.
                let same = xs.iter().zip(&f.values[c]).all(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) if k == 1.0 => a == b,
                    (Some(a), Some(b)) => converted_equal(*a, b * k),
                    (None, None) => true,
                    _ => false,
                });
                if !same {
                    return Err(apart("their x values differ".into()));
                }
            }
            (None, None) => {}
            _ => return Err(apart("one has x, the other has not".into())),
        }
    }
    if base.is_none() && reads.contains(&Read::Scalar) {
        len = 1;
    }
    // Series without points still have their x, without values where only
    // scalars or frames without x give the points.
    if x.is_none() {
        x = frames.iter().find_map(|f| {
            f.column("x")
                .map(|c| (f.columns[c].clone(), vec![None; len]))
        });
    }
    Ok(Aligned { len, x, reads })
}

/// True if two x values are equal up to the rounding of one unit
/// conversion: a few units in the last place, not a relative tolerance
/// that grows past the spacing of the points. A conversion that
/// overflowed equals nothing: its difference and the tolerance would both
/// be infinite.
fn converted_equal(a: f64, b: f64) -> bool {
    a.is_finite()
        && b.is_finite()
        && (a == b || (a - b).abs() <= 4.0 * f64::EPSILON * a.abs().max(b.abs()))
}

/// The attributes that every frame with a value at a point has the same
/// at that point (a scalar at its one point), other than those in
/// `common`, which the frame keeps; `None` if there are none.
/// `has_value(k, i)` tells whether frame `k` gives a value at point `i`.
fn shared_point_attrs(
    frames: &[Frame],
    al: &Aligned,
    common: &Attributes,
    has_value: &dyn Fn(usize, usize) -> bool,
) -> Option<Vec<Attributes>> {
    // The names points can have: those of the frames and their points, not
    // shared by all frames.
    let mut names: Vec<&String> = (0..frames.len())
        .filter(|&k| al.reads[k] != Read::Empty)
        .flat_map(|k| {
            let f = &frames[k];
            f.attrs
                .keys()
                .chain(f.point_attrs.iter().flatten().flat_map(|p| p.keys()))
        })
        .filter(|name| !common.contains_key(*name))
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return None;
    }
    let points: Vec<Attributes> = (0..al.len)
        .map(|i| {
            let contributing: Vec<(usize, usize)> = (0..frames.len())
                .filter(|&k| has_value(k, i))
                .filter_map(|k| Some((k, al.point(k, i)?)))
                .collect();
            let Some((&(first, p), rest)) = contributing.split_first() else {
                return Attributes::new();
            };
            names
                .iter()
                .filter_map(|&name| {
                    let value = frames[first].point_attr(p, name)?;
                    rest.iter()
                        .all(|&(k, q)| frames[k].point_attr(q, name).is_some_and(|b| b.same(value)))
                        .then(|| (name.clone(), value.clone()))
                })
                .collect()
        })
        .collect();
    points.iter().any(|p| !p.is_empty()).then_some(points)
}

// combine --------------------------------------------------------------------------------

/// The inputs of the match whose level a combine merges.
#[derive(Debug, Clone, Default)]
pub struct MatchInputs {
    /// The names, `a` first.
    pub names: Vec<String>,
    /// The columns each input had when the match ran, where the nodes up to
    /// the combine keep them: the columns of an input none of whose frames
    /// found a partner.
    pub columns: BTreeMap<String, Vec<Column>>,
}

/// The frames of each innermost group become one frame, point by point,
/// so that level disappears: with functions across the frames, or with
/// expressions over the frames of a match. `inputs` are the inputs of the
/// match, if the run found the one the combine reads.
pub fn combine(
    id: &str,
    n: &CombineNode,
    tree: Tree,
    inputs: Option<&MatchInputs>,
    catalogue: &UnitCatalogue,
) -> Result<Tree> {
    if let Some(functions) = &n.functions {
        let column = match &n.column {
            Some(s) => Some(bound(id, "column", s)?.as_str()),
            None => None,
        };
        let ps = percentiles(functions);
        return merge_innermost(tree, &mut |frames| {
            combine_functions(id, functions, column, &ps, frames, catalogue)
        });
    }
    let expressions = n.expressions.as_deref().unwrap_or(&[]);
    let names = match inputs {
        Some(m) => m.names.clone(),
        None => seen_inputs(&tree),
    };
    // The columns of each input: from its frames, else from the match.
    let mut templates: BTreeMap<String, Vec<Column>> = BTreeMap::new();
    for f in tree.frames() {
        let name = f.input.clone().unwrap_or_else(|| "a".into());
        templates.entry(name).or_insert_with(|| f.columns.clone());
    }
    for (name, columns) in inputs.map(|m| &m.columns).into_iter().flatten() {
        templates
            .entry(name.clone())
            .or_insert_with(|| columns.clone());
    }
    let mut exprs = Vec::with_capacity(expressions.len());
    for (i, e) in expressions.iter().enumerate() {
        exprs.push(Formula::new(id, i, e, &names, &templates, catalogue)?);
    }
    let mut c = Combiner {
        id,
        names: &names,
        templates: &templates,
        exprs,
        catalogue,
    };
    merge_innermost(tree, &mut |frames| c.group(frames))
}

/// The names of the inputs of the frames, `a` first, then in the order
/// the frames have them.
fn seen_inputs(tree: &Tree) -> Vec<String> {
    let mut names = vec!["a".to_owned()];
    for f in tree.frames() {
        if let Some(name) = &f.input
            && !names.contains(name)
        {
            names.push(name.clone());
        }
    }
    names
}

fn combine_functions(
    id: &str,
    functions: &[Function],
    column: Option<&str>,
    ps: &[f64],
    frames: Vec<Frame>,
    catalogue: &UnitCatalogue,
) -> Result<Option<Frame>> {
    if frames.is_empty() {
        return Ok(None);
    }
    let al = align(id, &frames, catalogue)?;
    // The column of each frame and the factor to the unit of the first.
    let mut reads: Vec<(usize, f64)> = Vec::with_capacity(frames.len());
    let mut unit: Option<(String, String)> = None;
    for (k, f) in frames.iter().enumerate() {
        if al.reads[k] == Read::Empty && f.columns.is_empty() {
            reads.push((0, 1.0));
            continue;
        }
        let c = value_column(id, f, column)?;
        let col = &f.columns[c];
        let k = match &unit {
            None => {
                unit = Some((col.name.clone(), col.unit.clone()));
                1.0
            }
            Some((name, to)) => factor(id, catalogue, name, &col.unit, to)?,
        };
        reads.push((c, k));
    }
    let unit = unit.map(|u| u.1).unwrap_or_default();
    let outputs = function_columns(functions, &unit);
    let mut values: Vec<Vec<Option<f64>>> = vec![Vec::with_capacity(al.len); outputs.len()];
    // The value of frame `k` at point `i` in the unit of the first; one
    // that a conversion takes out of range is no value, here and for the
    // attributes the point shares.
    let read = |k: usize, i: usize| {
        let (c, f) = reads[k];
        al.value(&frames, k, c, i)
            .map(|v| v * f)
            .filter(|v| v.is_finite())
    };
    let mut at = Vec::with_capacity(frames.len());
    for i in 0..al.len {
        at.clear();
        at.extend((0..frames.len()).map(|k| read(k, i)));
        let s = stats::summarize(&at, ps, DEFAULT_CONFIDENCE);
        for ((_, what), out) in outputs.iter().zip(&mut values) {
            out.push(compute(what, &s, &at));
        }
    }
    let mut columns = Vec::with_capacity(outputs.len() + 1);
    if let Some((x, xs)) = &al.x {
        columns.push(x.clone());
        values.insert(0, xs.clone());
    }
    columns.extend(outputs.into_iter().map(|(c, _)| c));
    let mut common = common_attributes(&frames);
    // An attribute that a point overrides is shared only where the points
    // agree.
    for p in frames
        .iter()
        .filter_map(|f| f.point_attrs.as_ref())
        .flatten()
    {
        common.retain(|name, _| !p.contains_key(name));
    }
    let has_value = |k: usize, i: usize| read(k, i).is_some();
    let points = shared_point_attrs(&frames, &al, &common, &has_value);
    let mut frame = Frame::new(common, columns, values);
    frame.point_attrs = points;
    frame.input = common_input(&frames);
    Ok(Some(frame))
}

/// What a name of a combine expression reads.
#[derive(Debug, Clone)]
enum Ref {
    /// A column of input `input` (an index into the input names).
    Column { input: usize, column: String },
    /// An attribute of a partner: `b.run.id`.
    Partner { input: usize, attr: String },
    /// An attribute of `a`'s point.
    Own(String),
    /// A name of an input without frames: no value.
    Absent,
}

/// The value columns of an input, as the combined frame names them: the
/// input's name for its only column other than `x`, else
/// `<input>.<column>`.
fn value_columns<'c>(name: &str, columns: &'c [Column]) -> Vec<(String, &'c Column)> {
    let values: Vec<&Column> = columns.iter().filter(|c| c.name != "x").collect();
    match values.as_slice() {
        [one] => vec![(name.to_owned(), *one)],
        many => many
            .iter()
            .map(|c| (format!("{name}.{}", c.name), *c))
            .collect(),
    }
}

/// Resolves a name of a combine expression: `<input>` or
/// `<input>.<column>` is a column, `<other input>.<attribute>` an
/// attribute of that partner, any other attribute one of `a`'s point.
fn resolve(
    name: &str,
    names: &[String],
    templates: &BTreeMap<String, Vec<Column>>,
) -> std::result::Result<Ref, String> {
    let (head, rest) = match name.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (name, None),
    };
    if let Some(input) = names.iter().position(|n| n == head) {
        let Some(columns) = templates.get(head) else {
            return Ok(Ref::Absent);
        };
        let list = || {
            let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
            names.join(", ")
        };
        return match rest {
            None => match value_columns(head, columns).as_slice() {
                [(_, one)] => Ok(Ref::Column {
                    input,
                    column: one.name.clone(),
                }),
                _ => Err(format!(
                    "{head} has the columns {}: name one, e.g. {head}.{}",
                    list(),
                    columns
                        .iter()
                        .find(|c| c.name != "x")
                        .map_or("x", |c| c.name.as_str())
                )),
            },
            Some(r) if columns.iter().any(|c| c.name == r) => Ok(Ref::Column {
                input,
                column: r.to_owned(),
            }),
            Some(r) if input > 0 && attributes::lookup(attributes::unqualified(r)).is_some() => {
                Ok(Ref::Partner {
                    input,
                    attr: r.to_owned(),
                })
            }
            Some(r) => Err(format!(
                "{head} has no column {r}; its columns are {}",
                list()
            )),
        };
    }
    if attributes::lookup(attributes::unqualified(name)).is_some() {
        return Ok(Ref::Own(name.to_owned()));
    }
    Err(format!(
        "unknown name {name}: name an input ({}), one of its columns (b.mean) or an attribute",
        names.join(", ")
    ))
}

/// One expression of a combine.
struct Formula<'a> {
    index: usize,
    name: &'a str,
    declared: Option<&'a str>,
    expr: Expr,
    /// The distinct names the expression reads, sorted, and what they read.
    names: Vec<String>,
    refs: Vec<Ref>,
    /// The checked expression for each combination of the units of its
    /// names.
    checked: HashMap<Vec<String>, Checked>,
    /// The unit of the column where no point gives one: the declared unit,
    /// or the unit of the expression over the columns of the inputs.
    default_unit: Option<String>,
    /// For each name, its unit over the columns of the inputs and the
    /// fixed attributes, where that is known: the unit of a name that a
    /// group lacks, such as an attribute of a missing partner.
    template_units: Vec<Option<String>>,
}

impl<'a> Formula<'a> {
    fn new(
        id: &str,
        index: usize,
        e: &'a Expression,
        names: &[String],
        templates: &BTreeMap<String, Vec<Column>>,
        catalogue: &UnitCatalogue,
    ) -> Result<Self> {
        let setting = format!("expressions[{index}].expression");
        let expr =
            parse_columns(&e.expression).map_err(|d| Problem::expression(id, &setting, &d))?;
        let mut found = expr.attributes();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found.dedup_by(|a, b| a.0 == b.0);
        let mut refs = Vec::with_capacity(found.len());
        for (name, span) in &found {
            let r = resolve(name, names, templates)
                .map_err(|m| Problem::expression(id, &setting, &Diagnostic::new(*span, m)))?;
            refs.push(r);
        }
        let mut f = Formula {
            index,
            name: &e.name,
            declared: e.unit.as_deref(),
            expr,
            names: found.into_iter().map(|(n, _)| n).collect(),
            refs,
            checked: HashMap::new(),
            default_unit: e.unit.clone(),
            template_units: Vec::new(),
        };
        // Over columns and fixed attributes, which have no unit, the unit
        // is known before any point: the unit of a column where no point
        // gives one. Only a fallback; points are checked with their units.
        let fixed = |name: &str| {
            let name = attributes::unqualified(name);
            attributes::FIXED
                .iter()
                .any(|(n, ..)| *n == name)
                .then(String::new)
        };
        f.template_units = f
            .refs
            .iter()
            .map(|r| match r {
                Ref::Column { input, column } => templates
                    .get(&names[*input])?
                    .iter()
                    .find(|c| &c.name == column)
                    .map(|c| c.unit.clone()),
                Ref::Partner { attr, .. } => fixed(attr),
                Ref::Own(name) => fixed(name),
                Ref::Absent => None,
            })
            .collect();
        if f.default_unit.is_none() {
            let units: Option<Vec<String>> = f.template_units.iter().cloned().collect();
            if let Some(units) = units
                && f.check(id, &units, catalogue).is_ok()
            {
                f.default_unit = Some(f.checked[&units].signature.unit.clone());
            }
        }
        Ok(f)
    }

    /// The expression checked for the units of its names. Units that do
    /// not fit it are an error of the node: a silent no value would look
    /// like the no value of FR-12a.
    fn check(&mut self, id: &str, units: &[String], catalogue: &UnitCatalogue) -> Result<()> {
        if !self.checked.contains_key(units) {
            let mut env = NameEnv {
                names: &self.names,
                units,
            };
            let checked =
                check(&self.expr, &mut env, catalogue, self.declared).map_err(|diagnostics| {
                    let d = diagnostics.into_iter().next().unwrap_or_else(|| {
                        Diagnostic::new(self.expr.span, "the expression does not check")
                    });
                    Problem::expression(id, &format!("expressions[{}].expression", self.index), &d)
                })?;
            let unit = &checked.signature.unit;
            if grown(unit, units, self.declared) {
                return Err(Problem::setting(
                    id,
                    format!("expressions[{}].unit", self.index),
                    format!(
                        "the unit of the expression is {} characters long, more than {}; \
                         declare a unit",
                        unit.chars().count(),
                        super::derive::MAX_UNIT_LEN
                    ),
                ));
            }
            self.checked.insert(units.to_vec(), checked);
        }
        Ok(())
    }
}

struct Combiner<'a> {
    id: &'a str,
    names: &'a [String],
    templates: &'a BTreeMap<String, Vec<Column>>,
    exprs: Vec<Formula<'a>>,
    catalogue: &'a UnitCatalogue,
}

impl Combiner<'_> {
    /// Combines the frames of one match group into one frame.
    fn group(&mut self, frames: Vec<Frame>) -> Result<Option<Frame>> {
        if frames.is_empty() {
            return Ok(None);
        }
        let id = self.id;
        let al = align(id, &frames, self.catalogue)?;
        // The frame of each input in the group.
        let of: Vec<Option<usize>> = self
            .names
            .iter()
            .map(|name| {
                frames
                    .iter()
                    .position(|f| f.input.as_deref().unwrap_or("a") == name)
            })
            .collect();
        let a = of.first().copied().flatten();

        let mut columns = Vec::new();
        let mut values: Vec<Vec<Option<f64>>> = Vec::new();
        if let Some((x, xs)) = &al.x {
            columns.push(x.clone());
            values.push(xs.clone());
        } else if let Some(x) = self
            .names
            .iter()
            .filter_map(|name| self.templates.get(name))
            .find_map(|cs| cs.iter().find(|c| c.name == "x"))
        {
            // A series without a partner here still gives the x column.
            columns.push(x.clone());
            values.push(vec![None; al.len]);
        }
        // The inputs side by side.
        for (k, name) in self.names.iter().enumerate() {
            let Some(template) = self.templates.get(name) else {
                continue;
            };
            for (label, col) in value_columns(name, template) {
                let read = of[k].and_then(|f| Some((f, frames[f].column(&col.name)?)));
                let unit =
                    read.map_or(col.unit.clone(), |(f, c)| frames[f].columns[c].unit.clone());
                columns.push(Column::new(label, unit));
                values.push(
                    (0..al.len)
                        .map(|i| read.and_then(|(f, c)| al.value(&frames, f, c, i)))
                        .collect(),
                );
            }
        }
        // The expressions.
        for e in 0..self.exprs.len() {
            let (unit, column) = self.evaluate(e, &frames, &al, &of)?;
            columns.push(Column::new(self.exprs[e].name, unit));
            values.push(column);
        }

        // Attributes: a's, and each partner's under its name.
        let mut attrs = Attributes::new();
        let mut point_attrs: Vec<Attributes> = vec![Attributes::new(); al.len];
        if let Some(a) = a {
            attrs.extend(frames[a].attrs.clone());
            match al.reads[a] {
                Read::Points => {
                    if let Some(p) = &frames[a].point_attrs {
                        point_attrs = p.clone();
                    }
                }
                Read::Scalar => {
                    if let Some(p) = frames[a].point_attrs.as_ref().and_then(|p| p.first()) {
                        attrs.extend(p.clone());
                    }
                }
                Read::Empty => {}
            }
        }
        // Attributes of partners that a's frame has from an earlier
        // combine give way to those of the partners here.
        for name in self.names.iter().skip(1) {
            let prefix = format!("{name}.");
            attrs.retain(|n, _| !n.starts_with(&prefix));
            for p in &mut point_attrs {
                p.retain(|n, _| !n.starts_with(&prefix));
            }
        }
        for (k, name) in self.names.iter().enumerate().skip(1) {
            let Some(f) = of[k] else { continue };
            let mut own = frames[f].attrs.clone();
            match (al.reads[f], &frames[f].point_attrs) {
                (Read::Scalar, Some(p)) => own.extend(p.first().cloned().unwrap_or_default()),
                // The partner's points keep their own attributes.
                (Read::Points, Some(p)) => {
                    for (out, mine) in point_attrs.iter_mut().zip(p) {
                        out.extend(mine.iter().map(|(n, v)| (format!("{name}.{n}"), v.clone())));
                    }
                }
                _ => {}
            }
            attrs.extend(own.into_iter().map(|(n, v)| (format!("{name}.{n}"), v)));
        }
        let mut frame = Frame::new(attrs, columns, values);
        if point_attrs.iter().any(|p| !p.is_empty()) {
            frame.point_attrs = Some(point_attrs);
        }
        Ok(Some(frame))
    }

    /// The unit and the values of expression `e` at every point.
    fn evaluate(
        &mut self,
        e: usize,
        frames: &[Frame],
        al: &Aligned,
        of: &[Option<usize>],
    ) -> Result<(String, Vec<Option<f64>>)> {
        let id = self.id;
        let catalogue = self.catalogue;
        let a = of.first().copied().flatten();
        let mut unit: Option<String> = None;
        let mut out = Vec::with_capacity(al.len);
        let n = self.exprs[e].refs.len();
        let mut units: Vec<Option<String>> = Vec::with_capacity(n);
        let mut inputs: Vec<f64> = Vec::with_capacity(n);
        for i in 0..al.len {
            units.clear();
            inputs.clear();
            for r in &self.exprs[e].refs {
                let (u, v) = self.read(r, frames, al, of, a, i);
                units.push(u);
                inputs.push(v.unwrap_or(f64::NAN));
            }
            // Without the unit of a name (a missing attribute, an input
            // without frames) there is no value to check.
            let Some(known) = units.iter().cloned().collect::<Option<Vec<String>>>() else {
                out.push(None);
                continue;
            };
            let f = &mut self.exprs[e];
            f.check(id, &known, catalogue)?;
            let checked = &f.checked[&known];
            // The unit is known once the units of the names are, also where
            // the point has no value.
            let point_unit = &checked.signature.unit;
            let k = match &unit {
                None => {
                    unit = Some(point_unit.clone());
                    1.0
                }
                Some(u) if u == point_unit => 1.0,
                Some(u) => catalogue.conversion_factor(point_unit, u).ok_or_else(|| {
                    Problem::setting(
                        id,
                        format!("expressions[{e}].expression"),
                        format!(
                            "the expression is in {point_unit:?} at one point and in {u:?} \
                                 at another, which cannot be converted; declare a unit"
                        ),
                    )
                })?,
            };
            let mut values = NameInputs {
                names: &f.names,
                values: &inputs,
            };
            let value = match eval(&checked.node, &mut values) {
                Ok(v) => v.scale(checked.output_factor),
                Err(_) => Value::Scalar(None),
            };
            let Value::Scalar(Some(v)) = value else {
                out.push(None);
                continue;
            };
            out.push(Some(v * k).filter(|v| v.is_finite()));
        }
        // No point gave a unit: the unit over the columns and frame
        // attributes of this group, else the fallback of the expression.
        // These units are the ones a point of this group would have, so
        // they are checked as a point's are: whether the expression is
        // accepted must not depend on whether the group has points. A name
        // that the group lacks (a missing partner) takes its unit over the
        // inputs, so the names the group has keep theirs; that check only
        // gives a unit, it does not fail the node, as those units need not
        // be this group's.
        if unit.is_none() {
            let mut own = true;
            let group: Option<Vec<String>> = self.exprs[e]
                .refs
                .iter()
                .zip(&self.exprs[e].template_units)
                .map(|(r, t)| {
                    self.frame_unit(r, frames, of, a).or_else(|| {
                        own = false;
                        t.clone()
                    })
                })
                .collect();
            if let Some(units) = group {
                let f = &mut self.exprs[e];
                match f.check(id, &units, catalogue) {
                    Ok(()) => unit = Some(f.checked[&units].signature.unit.clone()),
                    Err(err) if own => return Err(err),
                    Err(_) => {}
                }
            }
        }
        let unit = unit
            .or_else(|| self.exprs[e].default_unit.clone())
            .unwrap_or_default();
        Ok((unit, out))
    }

    /// The unit of a name in a group, from the columns and the frame
    /// attributes of its frames.
    fn frame_unit(
        &self,
        r: &Ref,
        frames: &[Frame],
        of: &[Option<usize>],
        a: Option<usize>,
    ) -> Option<String> {
        match r {
            Ref::Absent => None,
            Ref::Column { input, column } => {
                let f = &frames[of[*input]?];
                Some(f.columns[f.column(column)?].unit.clone())
            }
            Ref::Partner { input, attr } => Some(frames[of[*input]?].attrs.get(attr)?.unit.clone()),
            Ref::Own(name) => Some(frames[a?].attrs.get(name)?.unit.clone()),
        }
    }

    /// The unit and value of a name at point `i`. The unit is `None` where
    /// it is not known.
    fn read(
        &self,
        r: &Ref,
        frames: &[Frame],
        al: &Aligned,
        of: &[Option<usize>],
        a: Option<usize>,
        i: usize,
    ) -> (Option<String>, Option<f64>) {
        let attr = |a: Option<&Attr>| match a {
            Some(a) => (Some(a.unit.clone()), a.value.as_f64()),
            None => (None, None),
        };
        match r {
            Ref::Absent => (None, None),
            Ref::Column { input, column } => match of[*input] {
                Some(f) => match frames[f].column(column) {
                    Some(c) => (
                        Some(frames[f].columns[c].unit.clone()),
                        al.value(frames, f, c, i),
                    ),
                    None => (None, None),
                },
                // A missing partner: no value. Its unit in another group
                // need not be its unit here.
                None => (None, None),
            },
            Ref::Partner { input, attr: name } => match of[*input] {
                Some(f) => {
                    let frame = &frames[f];
                    let found = match al.point(f, i) {
                        Some(p) => frame.point_attr(p, name),
                        None => frame.attrs.get(name),
                    };
                    attr(found)
                }
                None => (None, None),
            },
            Ref::Own(name) => match a.and_then(|a| Some((a, al.point(a, i)?))) {
                Some((a, p)) => attr(frames[a].point_attr(p, name)),
                None => attr(a.and_then(|a| frames[a].attrs.get(name))),
            },
        }
    }
}

/// The units of the names of a combine expression at one point.
struct NameEnv<'a> {
    names: &'a [String],
    units: &'a [String],
}

impl Env for NameEnv<'_> {
    fn metric(&self, _: &str) -> Option<String> {
        None
    }

    fn series(&self, _: &str) -> Option<SeriesUnits> {
        None
    }

    fn derived(&mut self, name: &str) -> std::result::Result<Signature, String> {
        Err(format!("derived.{name} is a metric, not a column"))
    }

    fn attribute(&mut self, name: &str) -> std::result::Result<String, String> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| self.units[i].clone())
            .ok_or_else(|| format!("unknown name {name}"))
    }
}

/// The values of the names of a combine expression at one point; NaN is
/// no value.
struct NameInputs<'a> {
    names: &'a [String],
    values: &'a [f64],
}

impl Inputs for NameInputs<'_> {
    fn metric(&mut self, name: &str) -> std::result::Result<f64, EvalError> {
        Err(EvalError::Missing(name.to_owned()))
    }

    fn series(&mut self, name: &str) -> std::result::Result<Series, EvalError> {
        Err(EvalError::Missing(name.to_owned()))
    }

    fn derived(&mut self, name: &str) -> std::result::Result<Value, EvalError> {
        Err(EvalError::Missing(name.to_owned()))
    }

    fn attribute(&mut self, name: &str) -> std::result::Result<f64, EvalError> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| self.values[i])
            .ok_or_else(|| EvalError::Missing(name.to_owned()))
    }
}
