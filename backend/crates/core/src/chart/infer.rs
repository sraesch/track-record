//! The checks that make a well-formed spec valid (ADR 0022, "Spec"):
//! variables, values, the expressions of sources, attributes, and the
//! columns, units and group levels from node to node. A chart with such
//! problems can be saved, because a later change to a metric can cause them
//! anyway, but it cannot be run.
//!
//! Every node gets the [`Shape`] of its output: its group levels, its
//! columns with their units and the attributes its frames and points can
//! have. A setting that is a variable is checked with the variable's
//! default; without one, what depends on it is not known until the chart
//! runs. A node whose input could not be inferred gets no shape and no
//! further problems, so that one error does not cascade.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::attributes::{self, AttrType, Family};
use super::graph::Graph;
use super::problem::Problem;
use super::spec::{
    CombineNode, DEFAULT_LEVELS, DeriveNode, FlattenLevels, MatchNode, NodeKind, On, ReduceNode,
    RoleKind, SortTarget, SourceNode, Spec, is_identifier,
};
use super::variables::{Setting, check_variables};
use crate::dsl::check::{Env, Kind};
use crate::dsl::{self, Diagnostic};
use crate::units::UnitCatalogue;

/// A column of the frames of a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColumnShape {
    pub name: String,
    /// `""` for no unit; `None` if the unit is not known: it comes from a
    /// variable without default, or from an expression whose unit is not
    /// inferred.
    pub unit: Option<String>,
}

impl ColumnShape {
    fn new(name: impl Into<String>, unit: Option<String>) -> Self {
        Self {
            name: name.into(),
            unit,
        }
    }

    fn count(name: &str) -> Self {
        Self::new(name, Some(String::new()))
    }
}

/// One input of a match level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchInput {
    pub name: String,
    pub columns: Vec<ColumnShape>,
    pub attributes: BTreeSet<String>,
}

/// A group level, outermost first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Level {
    /// Made by a group node.
    Group {
        by: Vec<String>,
        except: Vec<String>,
    },
    /// Made by a match: each group holds a frame of `a` and its partners.
    Match { inputs: Vec<MatchInput> },
}

/// The output of a node as validation sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shape {
    pub levels: Vec<Level>,
    /// The columns of the frames; of the frames of `a` below a match level.
    pub columns: Vec<ColumnShape>,
    /// The attributes frames and points can have. `param.*` and `asset.*`
    /// stand for every parameter and asset; their names come from the data.
    pub attributes: BTreeSet<String>,
}

impl Shape {
    pub fn column(&self, name: &str) -> Option<&ColumnShape> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        has(&self.attributes, name)
    }

    fn column_names(&self) -> String {
        if self.columns.is_empty() {
            return "none".into();
        }
        let names: Vec<&str> = self.columns.iter().map(|c| c.name.as_str()).collect();
        names.join(", ")
    }
}

/// True if the attribute `name` is in `set`, directly or through a pattern
/// such as `param.*`.
fn has(set: &BTreeSet<String>, name: &str) -> bool {
    set.contains(name)
        || set.iter().any(|a| {
            a.strip_suffix('*')
                .is_some_and(|p| name.len() > p.len() && name.starts_with(p))
        })
}

/// The result of [`validate`].
#[derive(Debug, Clone, Default)]
pub struct Validation {
    pub problems: Vec<Problem>,
    /// The shape of each node whose inputs could be checked, by id.
    pub shapes: BTreeMap<String, Shape>,
}

/// Checks a well-formed spec. `env` gives the units of the metrics that
/// sources read.
pub fn validate(
    spec: &Spec,
    graph: &Graph,
    env: &mut dyn Env,
    catalogue: &UnitCatalogue,
) -> Validation {
    let mut problems = check_variables(spec);
    // Literals and defaults get the same value checks as bound values.
    let bound = spec.bind_defaults();
    for node in &bound.nodes {
        for (setting, message) in node.kind.check_values() {
            problems.push(Problem::setting(&node.id, setting, message));
        }
    }
    let mut shapes: Vec<Option<Shape>> = vec![None; spec.nodes.len()];
    for &i in graph.order() {
        let node = &bound.nodes[i];
        let inputs: Option<Vec<(Option<&str>, &Shape)>> = graph
            .inputs(i)
            .iter()
            .map(|(name, j)| shapes[*j].as_ref().map(|s| (name.as_deref(), s)))
            .collect();
        let Some(inputs) = inputs else {
            continue;
        };
        let mut cx = Cx {
            id: &node.id,
            problems: &mut problems,
            catalogue,
        };
        let shape = infer(&mut cx, env, &node.kind, &inputs);
        shapes[i] = shape;
    }
    let shapes = shapes
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|s| (spec.nodes[i].id.clone(), s)))
        .collect();
    Validation { problems, shapes }
}

struct Cx<'a> {
    id: &'a str,
    problems: &'a mut Vec<Problem>,
    catalogue: &'a UnitCatalogue,
}

impl Cx<'_> {
    fn problem(&mut self, setting: &str, message: impl Into<String>) {
        self.problems
            .push(Problem::setting(self.id, setting, message));
    }

    fn node_problem(&mut self, message: impl Into<String>) {
        self.problems.push(Problem::node(self.id, message));
    }

    fn expression(&mut self, setting: &str, d: &Diagnostic) {
        self.problems.push(Problem::expression(self.id, setting, d));
    }

    /// Checks that `name` is a key entry: an attribute or a family.
    fn key_entry(&mut self, shape: &Shape, setting: &str, name: &str) {
        if Family::from_key(name).is_none() && !shape.has_attribute(name) {
            self.problem(
                setting,
                format!(
                    "{name:?} is neither an attribute nor a family (run, case, device, sample, \
                     source)"
                ),
            );
        }
    }

    fn key_list(&mut self, shape: &Shape, setting: &str, names: Option<&Setting<Vec<String>>>) {
        if let Some(Setting::Value(names)) = names {
            for (i, name) in names.iter().enumerate() {
                self.key_entry(shape, &format!("{setting}[{i}]"), name);
            }
        }
    }

    /// Checks that `name` is what a role takes.
    fn role(&mut self, shape: &Shape, setting: &str, name: &str, kind: RoleKind) {
        let column = shape.column(name).is_some();
        let attribute = shape.has_attribute(name);
        let message = match kind {
            RoleKind::Column if column => return,
            RoleKind::Any if column || attribute => return,
            RoleKind::Attribute if attribute => return,
            RoleKind::Column if attribute => format!(
                "{name:?} is an attribute; this takes a column: {}",
                shape.column_names()
            ),
            RoleKind::Attribute if column => {
                format!("{name:?} is a column; this takes an attribute")
            }
            RoleKind::Column => format!(
                "there is no column {name:?}; the columns are {}",
                shape.column_names()
            ),
            RoleKind::Any => format!(
                "{name:?} is neither a column ({}) nor an attribute",
                shape.column_names()
            ),
            RoleKind::Attribute => format!("there is no attribute {name:?}"),
        };
        self.problem(setting, message);
    }

    /// The column a reduce or combine with functions reads: the one named,
    /// or the only one that is not `x` or a count.
    fn reduced_column(
        &mut self,
        shape: &Shape,
        column: Option<&Setting<String>>,
    ) -> Option<ColumnShape> {
        match column {
            Some(Setting::Var(_)) => Some(ColumnShape::new("", None)),
            Some(Setting::Value(name)) => {
                let found = shape.column(name).cloned();
                if found.is_none() {
                    self.problem(
                        "column",
                        format!(
                            "there is no column {name:?}; the columns are {}",
                            shape.column_names()
                        ),
                    );
                }
                found
            }
            None => {
                let candidates: Vec<&ColumnShape> = shape
                    .columns
                    .iter()
                    .filter(|c| c.name != "x" && !is_count(&c.name))
                    .collect();
                match candidates.as_slice() {
                    [one] => Some((*one).clone()),
                    [] => {
                        self.problem("column", "the frames have no column to compute with");
                        None
                    }
                    _ => {
                        self.problem(
                            "column",
                            format!(
                                "the frames have the columns {}: name one in column",
                                shape.column_names()
                            ),
                        );
                        None
                    }
                }
            }
        }
    }
}

/// Columns that count: they are never the default column of a reduce.
fn is_count(name: &str) -> bool {
    matches!(name, "n" | "n_no_value" | "count")
}

fn infer(
    cx: &mut Cx,
    env: &mut dyn Env,
    kind: &NodeKind,
    inputs: &[(Option<&str>, &Shape)],
) -> Option<Shape> {
    if let NodeKind::Source(s) = kind {
        return source(cx, env, s);
    }
    let input = inputs.first()?.1;
    match kind {
        NodeKind::Source(_) => None,
        NodeKind::Filter(n) => {
            for (i, c) in n.conditions.iter().enumerate() {
                let at = format!("conditions[{i}].attribute");
                match n.on {
                    On::Frames if input.column(&c.attribute).is_some() => cx.problem(
                        &at,
                        format!(
                            "{:?} is a column; conditions on columns filter points: set on to \
                             points",
                            c.attribute
                        ),
                    ),
                    On::Frames => cx.role(input, &at, &c.attribute, RoleKind::Attribute),
                    On::Points => cx.role(input, &at, &c.attribute, RoleKind::Any),
                }
            }
            Some(input.clone())
        }
        NodeKind::Derive(n) => {
            derive_expression(cx, n, input);
            let mut shape = input.clone();
            if is_identifier(&n.name) {
                let name = format!("attr.{}", n.name);
                if shape.attributes.contains(&name) {
                    cx.problem("name", format!("the attribute {name} exists already"));
                }
                shape.attributes.insert(name);
            }
            Some(shape)
        }
        NodeKind::Group(n) => {
            cx.key_list(input, "by", Some(&n.by));
            cx.key_list(input, "except", n.except.as_ref());
            let list = |s: Option<&Setting<Vec<String>>>| match s {
                Some(Setting::Value(v)) => v.clone(),
                _ => Vec::new(),
            };
            let mut shape = input.clone();
            shape.levels.push(Level::Group {
                by: list(Some(&n.by)),
                except: list(n.except.as_ref()),
            });
            Some(shape)
        }
        NodeKind::Flatten(n) => {
            let mut shape = input.clone();
            let depth = shape.levels.len();
            if depth == 0 {
                cx.node_problem("the input has no group level to flatten");
                return Some(shape);
            }
            let remove = match n.levels.unwrap_or(FlattenLevels::Count(1)) {
                FlattenLevels::All => depth,
                FlattenLevels::Count(k) => {
                    let k = k as usize;
                    if k > depth {
                        cx.problem(
                            "levels",
                            format!("the input has only {depth} group levels, not {k}"),
                        );
                    }
                    k.min(depth)
                }
            };
            for level in shape.levels.drain(depth - remove..) {
                let Level::Match { inputs } = level else {
                    continue;
                };
                // The frames of all inputs end up in one list: they need the
                // same columns, as a reduce or concatenate would need them.
                let (a, others) = inputs.split_first().expect("a match has inputs");
                if let Some(other) = others
                    .iter()
                    .find(|o| !same_columns(&a.columns, &o.columns))
                {
                    cx.problem(
                        "levels",
                        format!(
                            "flattening the match puts the frames of {} ({}) and {} ({}) into one \
                             list, but they have different columns: combine them instead",
                            a.name,
                            show_columns(&a.columns),
                            other.name,
                            show_columns(&other.columns)
                        ),
                    );
                    return None;
                }
                for o in others {
                    shape.attributes.extend(o.attributes.iter().cloned());
                }
            }
            Some(shape)
        }
        NodeKind::Sort(n) => {
            if n.target == SortTarget::Groups && input.levels.is_empty() {
                cx.problem("target", "the input has no groups to sort");
            }
            for (i, key) in n.by.iter().enumerate() {
                let kind = if n.target == SortTarget::Points {
                    RoleKind::Any
                } else {
                    RoleKind::Attribute
                };
                cx.role(input, &format!("by[{i}].key"), &key.key, kind);
            }
            Some(input.clone())
        }
        NodeKind::SelectLatest(_) => Some(input.clone()),
        NodeKind::Resample(_) => {
            if input.column("x").is_none() {
                cx.node_problem("resample needs frames with an x column: the points of a series");
            }
            Some(input.clone())
        }
        NodeKind::Match(n) => Some(match_node(cx, n, inputs)),
        NodeKind::Reduce(n) => reduce(cx, n, input),
        NodeKind::Concatenate(_) => {
            if matches!(input.levels.last(), Some(Level::Match { .. })) {
                cx.node_problem(
                    "concatenate needs frames with the same columns, but a match groups frames \
                     of different inputs: use combine",
                );
                return None;
            }
            let mut shape = input.clone();
            shape.levels.pop();
            Some(shape)
        }
        NodeKind::Combine(n) => combine(cx, n, input),
        NodeKind::Line(_)
        | NodeKind::Scatter(_)
        | NodeKind::Bar(_)
        | NodeKind::Box(_)
        | NodeKind::Heatmap(_)
        | NodeKind::Value(_)
        | NodeKind::Table(_) => {
            visualization(cx, kind, input);
            Some(input.clone())
        }
    }
}

fn source(cx: &mut Cx, env: &mut dyn Env, s: &SourceNode) -> Option<Shape> {
    // A metric from a variable without default: known when the chart runs.
    let Setting::Value(metric) = &s.metric else {
        return None;
    };
    let expr = match dsl::parse(metric) {
        Ok(expr) => expr,
        Err(d) => {
            cx.expression("metric", &d);
            return None;
        }
    };
    let declared = match &s.unit {
        Some(Setting::Value(u)) => Some(u.as_str()),
        _ => None,
    };
    let checked = match dsl::check::check(&expr, env, cx.catalogue, declared) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            for d in &diagnostics {
                cx.expression("metric", d);
            }
            return None;
        }
    };
    let sig = checked.signature;
    let mut columns = Vec::new();
    match sig.kind {
        Kind::Series => {
            let x_unit = match &s.x_unit {
                None => Some(sig.x_unit.clone()),
                Some(Setting::Var(_)) => None,
                Some(Setting::Value(u)) => {
                    if !cx.catalogue.convertible(&sig.x_unit, u) {
                        cx.problem(
                            "x_unit",
                            format!(
                                "x is in {:?}, which cannot be converted to {u:?}",
                                sig.x_unit
                            ),
                        );
                    }
                    Some(u.clone())
                }
            };
            columns.push(ColumnShape::new("x", x_unit));
        }
        Kind::Scalar => {
            if s.x_unit.is_some() {
                cx.problem(
                    "x_unit",
                    "the expression gives a scalar, which has no x; x_unit is for series",
                );
            }
        }
    }
    let y_unit = match &s.unit {
        Some(Setting::Var(_)) => None,
        _ => Some(sig.unit),
    };
    columns.push(ColumnShape::new("y", y_unit));
    Some(Shape {
        levels: Vec::new(),
        columns,
        attributes: attributes::source_attributes().collect(),
    })
}

/// Checks the expression of a derive node: its syntax, that its names are
/// numeric attributes of the input, and, where all their units are known
/// (the fixed attributes have none), its units. The units of parameters
/// and derived attributes come from the data; the run checks them per
/// frame.
fn derive_expression(cx: &mut Cx, n: &DeriveNode, input: &Shape) {
    if n.expression.trim().is_empty() {
        return; // reported with the values
    }
    let expr = match dsl::parse_attributes(&n.expression) {
        Ok(expr) => expr,
        Err(d) => return cx.expression("expression", &d),
    };
    let mut known = true;
    let mut ok = true;
    for (name, span) in expr.attributes() {
        let message = match attributes::lookup(attributes::unqualified(&name)) {
            _ if !input.has_attribute(&name) => format!("there is no attribute {name}"),
            Some(info) if matches!(info.ty, AttrType::Number | AttrType::Any) => {
                known &= attributes::FIXED.iter().any(|(f, ..)| *f == name);
                continue;
            }
            _ => format!("{name} is not a number; derive computes with numeric attributes"),
        };
        ok = false;
        cx.expression("expression", &Diagnostic::new(span, message));
    }
    if !(ok && known) {
        return;
    }
    let mut env = FixedAttributes;
    if let Err(diagnostics) = dsl::check::check(&expr, &mut env, cx.catalogue, n.unit.as_deref()) {
        for d in &diagnostics {
            cx.expression("expression", d);
        }
    }
}

/// The units of the fixed numeric attributes: none.
struct FixedAttributes;

impl Env for FixedAttributes {
    fn metric(&self, _: &str) -> Option<String> {
        None
    }

    fn series(&self, _: &str) -> Option<crate::dsl::check::SeriesUnits> {
        None
    }

    fn derived(&mut self, name: &str) -> Result<crate::dsl::Signature, String> {
        Err(format!("derived.{name} is a metric, not an attribute"))
    }

    fn attribute(&mut self, _: &str) -> Result<String, String> {
        Ok(String::new())
    }
}

fn match_node(cx: &mut Cx, n: &MatchNode, inputs: &[(Option<&str>, &Shape)]) -> Shape {
    let a = inputs[0].1;
    cx.key_list(a, "except", n.except.as_ref());
    cx.key_list(a, "only", n.only.as_ref());
    let mut shape = a.clone();
    shape.levels.push(Level::Match {
        inputs: inputs
            .iter()
            .map(|(name, s)| MatchInput {
                name: name.unwrap_or("a").to_owned(),
                columns: s.columns.clone(),
                attributes: s.attributes.clone(),
            })
            .collect(),
    });
    shape
}

/// Columns of functions over `column`: counts have no unit, the others the
/// unit of the column. Two functions with the same column are a problem.
fn function_columns(
    cx: &mut Cx,
    functions: &[super::spec::Function],
    column: &ColumnShape,
    out: &mut Vec<ColumnShape>,
) {
    for (i, f) in functions.iter().enumerate() {
        let name = f.column();
        if out.iter().any(|c| c.name == name) || is_count(&name) && !f.is_count() {
            cx.problem(
                &format!("functions[{i}]"),
                format!("two functions give the column {name}"),
            );
            continue;
        }
        let unit = if f.is_count() {
            Some(String::new())
        } else {
            column.unit.clone()
        };
        out.push(ColumnShape::new(name, unit));
    }
}

fn reduce(cx: &mut Cx, n: &ReduceNode, input: &Shape) -> Option<Shape> {
    if matches!(input.levels.last(), Some(Level::Match { .. })) {
        cx.node_problem(
            "reduce merges the frames of a group, but a match groups frames of different \
             inputs: combine them first",
        );
        return None;
    }
    let column = cx.reduced_column(input, n.column.as_ref())?;
    let mut columns = Vec::new();
    function_columns(cx, &n.functions, &column, &mut columns);
    for count in ["n", "n_no_value"] {
        if !columns.iter().any(|c| c.name == count) {
            columns.push(ColumnShape::count(count));
        }
    }
    let mut shape = input.clone();
    shape.levels.pop();
    shape.columns = columns;
    Some(shape)
}

fn combine(cx: &mut Cx, n: &CombineNode, input: &Shape) -> Option<Shape> {
    let last = input.levels.last();
    let mut shape = input.clone();
    shape.levels.pop();
    let mut columns: Vec<ColumnShape> = input.column("x").cloned().into_iter().collect();
    if let Some(functions) = &n.functions {
        if matches!(last, Some(Level::Match { .. })) {
            cx.node_problem(
                "combine with functions merges the frames of a group; after a match, combine \
                 the inputs with expressions",
            );
            return None;
        }
        let column = cx.reduced_column(input, n.column.as_ref())?;
        function_columns(cx, functions, &column, &mut columns);
        shape.columns = columns;
        return Some(shape);
    }
    let Some(Level::Match { inputs }) = last else {
        cx.node_problem(
            "combine with expressions needs a match: the expressions name its inputs (a, b, ...)",
        );
        return None;
    };
    // A frame with one point and no x applies to every point of the others,
    // so x comes from the inputs that are series, whichever their names.
    let mut xs = inputs
        .iter()
        .filter_map(|mi| mi.columns.iter().find(|c| c.name == "x").map(|c| (mi, c)));
    if let Some((first, x)) = xs.next() {
        for (mi, other) in xs {
            if let (Some(u), Some(v)) = (&x.unit, &other.unit)
                && !cx.catalogue.convertible(u, v)
            {
                cx.node_problem(format!(
                    "x of {} is in {u:?} and x of {} in {v:?}, which cannot be converted",
                    first.name, mi.name
                ));
            }
        }
        columns.push(x.clone());
    }
    for mi in inputs {
        let values: Vec<&ColumnShape> = mi.columns.iter().filter(|c| c.name != "x").collect();
        match values.as_slice() {
            [one] => columns.push(ColumnShape::new(&mi.name, one.unit.clone())),
            many => columns.extend(
                many.iter()
                    .map(|c| ColumnShape::new(format!("{}.{}", mi.name, c.name), c.unit.clone())),
            ),
        }
    }
    for (i, e) in n.expressions.iter().flatten().enumerate() {
        if columns.iter().any(|c| c.name == e.name) {
            cx.problem(
                &format!("expressions[{i}].name"),
                format!(
                    "the column {} exists already; the inputs give {}",
                    e.name,
                    columns
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            continue;
        }
        // The unit of the expression is inferred once expressions can read
        // the inputs (#29); until then it is the declared one.
        columns.push(ColumnShape::new(&e.name, e.unit.clone()));
    }
    let mut attributes = inputs[0].attributes.clone();
    for mi in &inputs[1..] {
        attributes.extend(mi.attributes.iter().map(|a| format!("{}.{a}", mi.name)));
    }
    shape.columns = columns;
    shape.attributes = attributes;
    Some(shape)
}

fn visualization(cx: &mut Cx, kind: &NodeKind, input: &Shape) {
    let viz = kind.visualization().expect("a visualization");
    let roles = viz.levels.map_or(DEFAULT_LEVELS.len(), <[_]>::len);
    let depth = input.levels.len();
    if depth > roles {
        cx.problem(
            "levels",
            format!(
                "the input has {depth} group levels, but only {roles} can become panels or \
                 colors: flatten or merge them first"
            ),
        );
    }
    for (name, setting, kind) in &viz.roles {
        if let Setting::Value(v) = setting {
            cx.role(input, name, v, *kind);
        }
    }
    // Bounds and comparisons are drawn on the axis of the value they
    // belong to, so their units must convert to its unit.
    for (role, of) in [
        ("low", "y"),
        ("high", "y"),
        ("low", "value"),
        ("high", "value"),
        ("compare", "value"),
    ] {
        let column = |name: &str| {
            viz.roles
                .iter()
                .find(|(n, ..)| *n == name)
                .and_then(|(_, s, _)| s.value())
                .and_then(|c| input.column(c))
        };
        if let (Some(c), Some(main)) = (column(role), column(of))
            && let (Some(u), Some(v)) = (&c.unit, &main.unit)
            && !cx.catalogue.convertible(u, v)
        {
            cx.problem(
                role,
                format!(
                    "{} is in {u:?} and {of} ({}) in {v:?}, which cannot be converted",
                    c.name, main.name
                ),
            );
        }
    }
    for (name, setting, kind) in &viz.lists {
        if let Setting::Value(list) = setting {
            for (i, v) in list.iter().enumerate() {
                let at = format!("{name}[{i}]");
                cx.role(input, &at, v, *kind);
                if *name == "differs" && !has_partner(input, v) {
                    cx.problem(
                        &at,
                        format!(
                            "differs compares {v} with the same attribute of a matched partner \
                             (b.{v}), and the input has none: combine the inputs of a match first"
                        ),
                    );
                }
            }
        }
    }
    for (role, axis) in viz.axes.into_iter().flatten() {
        let Some((_, setting, kind)) = viz.roles.iter().find(|(n, ..)| n == role) else {
            let names: Vec<&str> = viz.roles.iter().map(|(n, ..)| *n).collect();
            cx.problem(
                &format!("axes.{role}"),
                format!("there is no axis {role}; the axes are {}", names.join(", ")),
            );
            continue;
        };
        let (Some(Setting::Value(unit)), Setting::Value(column)) = (&axis.unit, setting) else {
            continue;
        };
        if *kind == RoleKind::Attribute {
            continue;
        }
        if let Some(ColumnShape {
            unit: Some(from), ..
        }) = input.column(column)
            && !cx.catalogue.convertible(from, unit)
        {
            cx.problem(
                &format!("axes.{role}.unit"),
                format!("{column} is in {from:?}, which cannot be converted to {unit:?}"),
            );
        }
    }
}

/// True if two inputs of a match have the same columns: names, and units
/// where both are known.
fn same_columns(a: &[ColumnShape], b: &[ColumnShape]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.name == y.name
                && match (&x.unit, &y.unit) {
                    (Some(u), Some(v)) => u == v,
                    _ => true,
                }
        })
}

fn show_columns(columns: &[ColumnShape]) -> String {
    let shown: Vec<String> = columns
        .iter()
        .map(|c| match c.unit.as_deref() {
            Some("") | None => c.name.clone(),
            Some(u) => format!("{} in {u}", c.name),
        })
        .collect();
    shown.join(", ")
}

/// True if the input has `<input>.<name>` for some input of a match that
/// a combine merged.
fn has_partner(shape: &Shape, name: &str) -> bool {
    shape.attributes.iter().any(|a| {
        a.split_once('.').is_some_and(|(prefix, rest)| {
            is_identifier(prefix)
                && !attributes::is_prefix(prefix)
                && (rest == name
                    || rest
                        .strip_suffix('*')
                        .is_some_and(|p| name.len() > p.len() && name.starts_with(p)))
        })
    })
}
