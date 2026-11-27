//! The JSON spec of a chart, format 1 (ADR 0022, "Spec", "Nodes",
//! "Visualizations").
//!
//! [`Spec::from_json`] reads a spec and reports unknown node types and
//! settings per node and setting. A spec serializes back to the same JSON,
//! variables included, so it can be stored and rewritten.

use std::collections::BTreeMap;
use std::ops::Not;

use serde::de::{DeserializeOwned, Error as _};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::FORMAT;
use super::filter;
use super::problem::Problem;
use super::variables::{Expect, Setting, VarType, Variable, Visit};
use crate::series::Interp;

/// A chart: its variables and the nodes of its graph.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Spec {
    pub format: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Variable>,
    pub nodes: Vec<Node>,
}

/// Where the editor shows a node. Evaluation ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// A node of the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    /// The one input of every node but sources and matches.
    pub input: Option<String>,
    /// The named inputs of a match: name → node id. `a` is the first.
    pub inputs: Option<BTreeMap<String, String>>,
    pub position: Option<Position>,
    pub kind: NodeKind,
}

impl Node {
    /// The ids of the nodes this one reads, with the names of a match's
    /// inputs; `a` first.
    pub fn input_ids(&self) -> Vec<(Option<&str>, &str)> {
        let mut out: Vec<(Option<&str>, &str)> = Vec::new();
        if let Some(input) = &self.input {
            out.push((None, input));
        }
        if let Some(inputs) = &self.inputs {
            if let Some(a) = inputs.get("a") {
                out.push((Some("a"), a));
            }
            out.extend(
                inputs
                    .iter()
                    .filter(|(name, _)| *name != "a")
                    .map(|(name, id)| (Some(name.as_str()), id.as_str())),
            );
        }
        out
    }
}

/// The run filter of a source: fields of the server's run filter, each a
/// literal or a variable.
pub type RunFilter = BTreeMap<String, Setting<Value>>;

/// The type of a node with its settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    Source(SourceNode),
    Filter(FilterNode),
    Derive(DeriveNode),
    Group(GroupNode),
    Flatten(FlattenNode),
    Sort(SortNode),
    SelectLatest(SelectLatestNode),
    Resample(ResampleNode),
    Match(MatchNode),
    Reduce(ReduceNode),
    Concatenate(ConcatenateNode),
    Combine(CombineNode),
    Line(LineChart),
    Scatter(ScatterChart),
    Bar(BarChart),
    Box(BoxChart),
    Heatmap(HeatmapChart),
    Value(ValueChart),
    Table(TableChart),
}

/// The names of the node types, visualizations last.
pub const NODE_TYPES: &[&str] = &[
    "source",
    "filter",
    "derive",
    "group",
    "flatten",
    "sort",
    "select_latest",
    "resample",
    "match",
    "reduce",
    "concatenate",
    "combine",
    "line",
    "scatter",
    "bar",
    "box",
    "heatmap",
    "value",
    "table",
];

/// Settings of every node, by the paths that the problems use.
macro_rules! settings {
    ($(#[$m:meta])* pub struct $name:ident { $($body:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $($body)* }
    };
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

settings! {
    /// A run filter and a metric expression: one frame per sample.
    pub struct SourceNode {
        /// A metric expression in the DSL (FR-17a).
        pub metric: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filter: Option<Setting<RunFilter>>,
        /// Unit of y; the expression's unit by default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub unit: Option<Setting<String>>,
        /// Unit of x for a series; the series' unit by default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub x_unit: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include_warmup: Option<Setting<bool>>,
    }
}

/// What a filter or a select_latest node works on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum On {
    #[default]
    Frames,
    Points,
}

/// The operator of a filter condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    #[serde(rename = "=")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "in")]
    In,
    #[serde(rename = "not in")]
    NotIn,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "exists")]
    Exists,
}

settings! {
    /// `attribute op value`. For points, `attribute` can also be a column.
    pub struct Condition {
        pub attribute: String,
        pub op: Op,
        /// Keeps its JSON type: `10` is not `"10"`. None for `exists`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<Setting<Value>>,
    }
}

settings! {
    /// Keeps the frames, or the points, that meet all conditions.
    pub struct FilterNode {
        #[serde(default, skip_serializing_if = "is_default")]
        pub on: On,
        pub conditions: Vec<Condition>,
    }
}

settings! {
    /// Adds `attr.<name>`, computed from numeric attributes.
    pub struct DeriveNode {
        pub name: String,
        pub expression: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub unit: Option<String>,
    }
}

settings! {
    /// Splits the frames of every innermost group by a key.
    pub struct GroupNode {
        /// Attributes and families.
        pub by: Setting<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub except: Option<Setting<Vec<String>>>,
    }
}

/// How many levels a flatten node removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlattenLevels {
    Count(u32),
    All,
}

impl Serialize for FlattenLevels {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            FlattenLevels::Count(n) => s.serialize_u32(*n),
            FlattenLevels::All => s.serialize_str("all"),
        }
    }
}

impl<'de> Deserialize<'de> for FlattenLevels {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Value::deserialize(d)? {
            Value::String(s) if s == "all" => Ok(FlattenLevels::All),
            Value::Number(n) => n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(FlattenLevels::Count)
                .ok_or_else(|| D::Error::custom("expected a number of levels or \"all\"")),
            _ => Err(D::Error::custom("expected a number of levels or \"all\"")),
        }
    }
}

settings! {
    /// Removes the innermost group level, or several, or all.
    pub struct FlattenNode {
        /// 1 by default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<FlattenLevels>,
    }
}

/// What a sort node orders.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortTarget {
    #[default]
    Frames,
    /// The groups of the innermost level.
    Groups,
    /// The points of each frame.
    Points,
}

settings! {
    pub struct SortKey {
        /// An attribute; for points also a column.
        pub key: String,
        #[serde(default, skip_serializing_if = "Not::not")]
        pub desc: bool,
    }
}

settings! {
    pub struct SortNode {
        #[serde(default, skip_serializing_if = "is_default")]
        pub target: SortTarget,
        pub by: Vec<SortKey>,
    }
}

settings! {
    /// Keeps the frames, or points, of the latest N runs.
    pub struct SelectLatestNode {
        /// 1 by default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub runs: Option<Setting<u32>>,
        #[serde(default, skip_serializing_if = "is_default")]
        pub target: On,
    }
}

/// Grid points of a resample node by default.
pub const DEFAULT_GRID_POINTS: u32 = 200;
/// Most grid points of a resample node.
pub const MAX_GRID_POINTS: u32 = 10_000;

settings! {
    /// Puts frames with an `x` column onto a common grid.
    pub struct ResampleNode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub points: Option<Setting<u32>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub from: Option<Setting<f64>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub to: Option<Setting<f64>>,
        #[serde(default, skip_serializing_if = "is_default")]
        pub interp: Interp,
    }
}

/// What a match node does with a frame without a partner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unmatched {
    #[default]
    Drop,
    Keep,
}

settings! {
    /// Pairs each frame of input `a` with the frames of the other inputs
    /// that have the same key.
    pub struct MatchNode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub except: Option<Setting<Vec<String>>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub only: Option<Setting<Vec<String>>>,
        #[serde(default, skip_serializing_if = "is_default")]
        pub unmatched: Unmatched,
    }
}

/// The functions of reduce and combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionName {
    Mean,
    Median,
    Min,
    Max,
    Sum,
    Stddev,
    Percentile,
    CiLow,
    CiHigh,
    Geomean,
    First,
    Last,
    Count,
}

impl FunctionName {
    /// The functions of a reduce node.
    pub const REDUCE: &[FunctionName] = &[
        FunctionName::Mean,
        FunctionName::Median,
        FunctionName::Min,
        FunctionName::Max,
        FunctionName::Sum,
        FunctionName::Stddev,
        FunctionName::Percentile,
        FunctionName::CiLow,
        FunctionName::CiHigh,
        FunctionName::Geomean,
        FunctionName::First,
        FunctionName::Last,
        FunctionName::Count,
    ];

    /// The functions of a combine node across the frames of a group.
    pub const COMBINE: &[FunctionName] = &[
        FunctionName::Mean,
        FunctionName::Median,
        FunctionName::Min,
        FunctionName::Max,
        FunctionName::Sum,
        FunctionName::Percentile,
        FunctionName::Count,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FunctionName::Mean => "mean",
            FunctionName::Median => "median",
            FunctionName::Min => "min",
            FunctionName::Max => "max",
            FunctionName::Sum => "sum",
            FunctionName::Stddev => "stddev",
            FunctionName::Percentile => "percentile",
            FunctionName::CiLow => "ci_low",
            FunctionName::CiHigh => "ci_high",
            FunctionName::Geomean => "geomean",
            FunctionName::First => "first",
            FunctionName::Last => "last",
            FunctionName::Count => "count",
        }
    }
}

/// A function of reduce or combine: `"mean"`, or
/// `{"function": "percentile", "p": 95}`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Function {
    pub name: FunctionName,
    /// The percentile, 0 to 100.
    pub p: Option<f64>,
}

impl Function {
    pub fn new(name: FunctionName) -> Self {
        Self { name, p: None }
    }

    pub fn percentile(p: f64) -> Self {
        Self {
            name: FunctionName::Percentile,
            p: Some(p),
        }
    }

    /// The column the function gives: its name, `p<p>` for a percentile
    /// (`p95`, `p99.9`).
    pub fn column(&self) -> String {
        match (self.name, self.p) {
            (FunctionName::Percentile, Some(p)) => format!("p{p}"),
            (name, _) => name.as_str().to_owned(),
        }
    }

    /// True if the function counts: its column has no unit.
    pub fn is_count(&self) -> bool {
        self.name == FunctionName::Count
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FunctionObject {
    function: FunctionName,
    #[serde(default)]
    p: Option<f64>,
}

impl Serialize for Function {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.p {
            None => self.name.serialize(s),
            Some(p) => {
                use serde::ser::SerializeMap;
                let mut m = s.serialize_map(Some(2))?;
                m.serialize_entry("function", &self.name)?;
                m.serialize_entry("p", &p)?;
                m.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Function {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        if value.is_string() {
            FunctionName::deserialize(value)
                .map(Function::new)
                .map_err(D::Error::custom)
        } else if value.is_object() {
            FunctionObject::deserialize(value)
                .map(|f| Function {
                    name: f.function,
                    p: f.p,
                })
                .map_err(D::Error::custom)
        } else {
            Err(D::Error::custom(
                "expected a function name or {\"function\": <name>, \"p\": <percentile>}",
            ))
        }
    }
}

/// Confidence of `ci_low` and `ci_high` by default.
pub const DEFAULT_CONFIDENCE: f64 = 0.95;

settings! {
    /// Turns every frame into one point.
    pub struct ReduceNode {
        /// The column to reduce; by default the only column that is not
        /// `x` or a count.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub column: Option<Setting<String>>,
        #[serde(default)]
        pub functions: Vec<Function>,
        /// Level of the confidence interval of `ci_low` and `ci_high`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub confidence: Option<Setting<f64>>,
    }
}

settings! {
    /// The frames of each innermost group become one frame with all their
    /// points.
    pub struct ConcatenateNode {}
}

settings! {
    /// A column that a combine node computes from the frames of a match.
    pub struct Expression {
        pub name: String,
        pub expression: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub unit: Option<String>,
    }
}

settings! {
    /// The frames of each innermost group become one frame, point by
    /// point: with `functions` across them, or with `expressions` over the
    /// frames of a match.
    pub struct CombineNode {
        /// The column the functions read; by default the only column that
        /// is not `x` or a count.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub column: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub functions: Option<Vec<Function>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expressions: Option<Vec<Expression>>,
    }
}

/// What a group level of the visualization becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelRole {
    /// One small chart per group.
    Panel,
    Color,
}

/// Roles of group levels by default, outermost first.
pub const DEFAULT_LEVELS: [LevelRole; 2] = [LevelRole::Panel, LevelRole::Color];

settings! {
    /// An axis: unit conversion within the catalogue and log scale (FR-13).
    pub struct Axis {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub unit: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Not::not")]
        pub log: bool,
    }
}

/// Axes by role (`x`, `y`, `value`, ...).
pub type Axes = BTreeMap<String, Axis>;

settings! {
    /// One line per frame.
    pub struct LineChart {
        pub x: Setting<String>,
        pub y: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub low: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub high: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub axes: Option<Axes>,
        /// Attributes whose changes from one point to the next are marked
        /// (FR-18).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub marks: Option<Setting<Vec<String>>>,
        /// Show the annotations of the runs, test cases and time range.
        #[serde(default, skip_serializing_if = "Not::not")]
        pub annotations: bool,
    }
}

settings! {
    /// One dot per point.
    pub struct ScatterChart {
        pub x: Setting<String>,
        pub y: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub axes: Option<Axes>,
    }
}

settings! {
    /// One bar per point.
    pub struct BarChart {
        pub category: Setting<String>,
        pub value: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub low: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub high: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub axes: Option<Axes>,
    }
}

settings! {
    /// One box per frame, from its points.
    pub struct BoxChart {
        pub category: Setting<String>,
        pub value: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub axes: Option<Axes>,
    }
}

settings! {
    /// One cell per point.
    pub struct HeatmapChart {
        pub x: Setting<String>,
        pub y: Setting<String>,
        pub value: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub axes: Option<Axes>,
    }
}

settings! {
    /// One number, optionally compared with another.
    pub struct ValueChart {
        pub value: Setting<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compare: Option<Setting<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub axes: Option<Axes>,
    }
}

settings! {
    /// One row per point.
    pub struct TableChart {
        /// Columns and attributes, in order.
        pub columns: Setting<Vec<String>>,
        /// An attribute whose values become columns (a pivot).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub across: Option<Setting<String>>,
        /// Attributes marked where they differ from the same attribute of a
        /// matched partner (`sut.id` and `b.sut.id`, FR-16).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub differs: Option<Setting<Vec<String>>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub levels: Option<Vec<LevelRole>>,
    }
}

/// What a role of a visualization takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleKind {
    /// A column.
    Column,
    /// A column or an attribute.
    Any,
    /// An attribute.
    Attribute,
}

/// The settings of a visualization, as the checks see them.
pub struct Visualization<'a> {
    /// Roles with one column or attribute.
    pub roles: Vec<(&'static str, &'a Setting<String>, RoleKind)>,
    /// Roles with a list of them.
    pub lists: Vec<(&'static str, &'a Setting<Vec<String>>, RoleKind)>,
    pub levels: Option<&'a [LevelRole]>,
    pub axes: Option<&'a Axes>,
}

fn role<'a>(
    out: &mut Vec<(&'static str, &'a Setting<String>, RoleKind)>,
    name: &'static str,
    s: &'a Option<Setting<String>>,
    kind: RoleKind,
) {
    if let Some(s) = s {
        out.push((name, s, kind));
    }
}

fn de<T: DeserializeOwned>(settings: Value) -> Result<T, String> {
    serde_json::from_value(settings).map_err(|e| e.to_string())
}

fn opt<T: Serialize + DeserializeOwned>(
    f: &mut Visit,
    path: &str,
    expect: Expect,
    s: &mut Option<Setting<T>>,
) {
    if let Some(s) = s {
        f(path, expect, s);
    }
}

const ATTR: Expect = Expect::one(VarType::Attribute);
const ATTRS: Expect = Expect::list(VarType::Attribute);
const TEXT: Expect = Expect::one(VarType::Text);
const NUMBER: Expect = Expect::one(VarType::Number);

impl NodeKind {
    /// Reads the settings of a node of type `ty`.
    pub fn from_settings(ty: &str, settings: Value) -> Result<NodeKind, String> {
        Ok(match ty {
            "source" => NodeKind::Source(de(settings)?),
            "filter" => NodeKind::Filter(de(settings)?),
            "derive" => NodeKind::Derive(de(settings)?),
            "group" => NodeKind::Group(de(settings)?),
            "flatten" => NodeKind::Flatten(de(settings)?),
            "sort" => NodeKind::Sort(de(settings)?),
            "select_latest" => NodeKind::SelectLatest(de(settings)?),
            "resample" => NodeKind::Resample(de(settings)?),
            "match" => NodeKind::Match(de(settings)?),
            "reduce" => NodeKind::Reduce(de(settings)?),
            "concatenate" => NodeKind::Concatenate(de(settings)?),
            "combine" => NodeKind::Combine(de(settings)?),
            "line" => NodeKind::Line(de(settings)?),
            "scatter" => NodeKind::Scatter(de(settings)?),
            "bar" => NodeKind::Bar(de(settings)?),
            "box" => NodeKind::Box(de(settings)?),
            "heatmap" => NodeKind::Heatmap(de(settings)?),
            "value" => NodeKind::Value(de(settings)?),
            "table" => NodeKind::Table(de(settings)?),
            other => {
                return Err(format!(
                    "unknown node type {other:?}; the types are {}",
                    NODE_TYPES.join(", ")
                ));
            }
        })
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            NodeKind::Source(_) => "source",
            NodeKind::Filter(_) => "filter",
            NodeKind::Derive(_) => "derive",
            NodeKind::Group(_) => "group",
            NodeKind::Flatten(_) => "flatten",
            NodeKind::Sort(_) => "sort",
            NodeKind::SelectLatest(_) => "select_latest",
            NodeKind::Resample(_) => "resample",
            NodeKind::Match(_) => "match",
            NodeKind::Reduce(_) => "reduce",
            NodeKind::Concatenate(_) => "concatenate",
            NodeKind::Combine(_) => "combine",
            NodeKind::Line(_) => "line",
            NodeKind::Scatter(_) => "scatter",
            NodeKind::Bar(_) => "bar",
            NodeKind::Box(_) => "box",
            NodeKind::Heatmap(_) => "heatmap",
            NodeKind::Value(_) => "value",
            NodeKind::Table(_) => "table",
        }
    }

    pub fn is_source(&self) -> bool {
        matches!(self, NodeKind::Source(_))
    }

    pub fn is_match(&self) -> bool {
        matches!(self, NodeKind::Match(_))
    }

    pub fn is_visualization(&self) -> bool {
        self.visualization().is_some()
    }

    /// The roles, levels and axes of a visualization; `None` for other
    /// nodes.
    pub fn visualization(&self) -> Option<Visualization<'_>> {
        use RoleKind::{Any, Attribute, Column};
        let mut roles = Vec::new();
        let mut lists = Vec::new();
        let (levels, axes) = match self {
            NodeKind::Line(c) => {
                roles.push(("x", &c.x, Any));
                roles.push(("y", &c.y, Column));
                role(&mut roles, "low", &c.low, Column);
                role(&mut roles, "high", &c.high, Column);
                if let Some(m) = &c.marks {
                    lists.push(("marks", m, Attribute));
                }
                (&c.levels, c.axes.as_ref())
            }
            NodeKind::Scatter(c) => {
                roles.push(("x", &c.x, Any));
                roles.push(("y", &c.y, Any));
                (&c.levels, c.axes.as_ref())
            }
            NodeKind::Bar(c) => {
                roles.push(("category", &c.category, Any));
                roles.push(("value", &c.value, Column));
                role(&mut roles, "low", &c.low, Column);
                role(&mut roles, "high", &c.high, Column);
                (&c.levels, c.axes.as_ref())
            }
            NodeKind::Box(c) => {
                roles.push(("category", &c.category, Any));
                roles.push(("value", &c.value, Column));
                (&c.levels, c.axes.as_ref())
            }
            NodeKind::Heatmap(c) => {
                roles.push(("x", &c.x, Any));
                roles.push(("y", &c.y, Any));
                roles.push(("value", &c.value, Column));
                (&c.levels, c.axes.as_ref())
            }
            NodeKind::Value(c) => {
                roles.push(("value", &c.value, Column));
                role(&mut roles, "compare", &c.compare, Column);
                (&c.levels, c.axes.as_ref())
            }
            NodeKind::Table(c) => {
                lists.push(("columns", &c.columns, Any));
                role(&mut roles, "across", &c.across, Attribute);
                if let Some(d) = &c.differs {
                    lists.push(("differs", d, Attribute));
                }
                (&c.levels, None)
            }
            _ => return None,
        };
        Some(Visualization {
            roles,
            lists,
            levels: levels.as_deref(),
            axes,
        })
    }

    /// Calls `f` for every setting that can stand for a variable.
    pub fn visit(&mut self, f: &mut Visit) {
        match self {
            NodeKind::Source(s) => {
                f("metric", Expect::one(VarType::Metric), &mut s.metric);
                if let Some(filter) = &mut s.filter {
                    f("filter", Expect::one(VarType::RunFilter), filter);
                    if let Setting::Value(fields) = filter {
                        for (name, value) in fields.iter_mut() {
                            if let Some(expect) = filter::field(name) {
                                f(&format!("filter.{name}"), expect, value);
                            }
                        }
                    }
                }
                opt(f, "unit", TEXT, &mut s.unit);
                opt(f, "x_unit", TEXT, &mut s.x_unit);
                opt(
                    f,
                    "include_warmup",
                    Expect::one(VarType::Boolean),
                    &mut s.include_warmup,
                );
            }
            NodeKind::Filter(s) => {
                for (i, c) in s.conditions.iter_mut().enumerate() {
                    opt(
                        f,
                        &format!("conditions[{i}].value"),
                        Expect::Value,
                        &mut c.value,
                    );
                }
            }
            NodeKind::Group(s) => {
                f("by", ATTRS, &mut s.by);
                opt(f, "except", ATTRS, &mut s.except);
            }
            NodeKind::SelectLatest(s) => opt(f, "runs", NUMBER, &mut s.runs),
            NodeKind::Resample(s) => {
                opt(f, "points", NUMBER, &mut s.points);
                opt(f, "from", NUMBER, &mut s.from);
                opt(f, "to", NUMBER, &mut s.to);
            }
            NodeKind::Match(s) => {
                opt(f, "except", ATTRS, &mut s.except);
                opt(f, "only", ATTRS, &mut s.only);
            }
            NodeKind::Reduce(s) => {
                opt(f, "column", ATTR, &mut s.column);
                opt(f, "confidence", NUMBER, &mut s.confidence);
            }
            NodeKind::Combine(s) => opt(f, "column", ATTR, &mut s.column),
            NodeKind::Line(c) => {
                f("x", ATTR, &mut c.x);
                f("y", ATTR, &mut c.y);
                opt(f, "low", ATTR, &mut c.low);
                opt(f, "high", ATTR, &mut c.high);
                opt(f, "marks", ATTRS, &mut c.marks);
                visit_axes(f, &mut c.axes);
            }
            NodeKind::Scatter(c) => {
                f("x", ATTR, &mut c.x);
                f("y", ATTR, &mut c.y);
                visit_axes(f, &mut c.axes);
            }
            NodeKind::Bar(c) => {
                f("category", ATTR, &mut c.category);
                f("value", ATTR, &mut c.value);
                opt(f, "low", ATTR, &mut c.low);
                opt(f, "high", ATTR, &mut c.high);
                visit_axes(f, &mut c.axes);
            }
            NodeKind::Box(c) => {
                f("category", ATTR, &mut c.category);
                f("value", ATTR, &mut c.value);
                visit_axes(f, &mut c.axes);
            }
            NodeKind::Heatmap(c) => {
                f("x", ATTR, &mut c.x);
                f("y", ATTR, &mut c.y);
                f("value", ATTR, &mut c.value);
                visit_axes(f, &mut c.axes);
            }
            NodeKind::Value(c) => {
                f("value", ATTR, &mut c.value);
                opt(f, "compare", ATTR, &mut c.compare);
                visit_axes(f, &mut c.axes);
            }
            NodeKind::Table(c) => {
                f("columns", ATTRS, &mut c.columns);
                opt(f, "across", ATTR, &mut c.across);
                opt(f, "differs", ATTRS, &mut c.differs);
            }
            NodeKind::Derive(_)
            | NodeKind::Flatten(_)
            | NodeKind::Sort(_)
            | NodeKind::Concatenate(_) => {}
        }
    }
}

fn visit_axes(f: &mut Visit, axes: &mut Option<Axes>) {
    for (role, axis) in axes.iter_mut().flatten() {
        opt(f, &format!("axes.{role}.unit"), TEXT, &mut axis.unit);
    }
}

/// The setting a serde error names, e.g. `foo` in "unknown field `foo`".
fn named_setting(message: &str) -> Option<String> {
    ["unknown field `", "missing field `", "duplicate field `"]
        .iter()
        .find_map(|p| message.find(p).map(|i| &message[i + p.len()..]))
        .and_then(|rest| rest.split('`').next())
        .map(str::to_owned)
}

const NODE_KEYS: [&str; 5] = ["id", "type", "input", "inputs", "position"];

impl Node {
    /// Reads node `index` of a spec.
    fn from_json(index: usize, value: &Value) -> Result<Node, Vec<Problem>> {
        let at = format!("nodes[{index}]");
        let Value::Object(object) = value else {
            return Err(vec![Problem::spec_setting(at, "a node is a JSON object")]);
        };
        let id = match object.get("id") {
            Some(Value::String(id)) if !id.trim().is_empty() => id.clone(),
            _ => {
                return Err(vec![Problem::spec_setting(
                    format!("{at}.id"),
                    "a node needs an id: a non-empty text",
                )]);
            }
        };
        let mut problems = Vec::new();
        let ty = match object.get("type") {
            Some(Value::String(ty)) => Some(ty.as_str()),
            _ => {
                problems.push(Problem::setting(
                    &id,
                    "type",
                    format!("a node needs a type: one of {}", NODE_TYPES.join(", ")),
                ));
                None
            }
        };
        let input = match object.get("input") {
            None => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => {
                problems.push(Problem::setting(&id, "input", "expected the id of a node"));
                None
            }
        };
        let inputs = match object.get("inputs") {
            None => None,
            Some(v) => match BTreeMap::<String, String>::deserialize(v) {
                Ok(m) => Some(m),
                Err(_) => {
                    problems.push(Problem::setting(
                        &id,
                        "inputs",
                        "expected names with the ids of nodes, e.g. {\"a\": \"runs\", \"b\": \"reference\"}",
                    ));
                    None
                }
            },
        };
        let position = match object.get("position") {
            None => None,
            Some(v) => match Position::deserialize(v) {
                Ok(p) => Some(p),
                Err(e) => {
                    problems.push(Problem::setting(&id, "position", e.to_string()));
                    None
                }
            },
        };
        let settings: Map<String, Value> = object
            .iter()
            .filter(|(k, _)| !NODE_KEYS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let kind = ty.and_then(
            |ty| match NodeKind::from_settings(ty, Value::Object(settings)) {
                Ok(kind) => Some(kind),
                Err(message) => {
                    let setting = if NODE_TYPES.contains(&ty) {
                        named_setting(&message)
                    } else {
                        Some("type".to_owned())
                    };
                    problems.push(Problem {
                        setting,
                        ..Problem::node(&id, message)
                    });
                    None
                }
            },
        );
        if let Some(NodeKind::Source(SourceNode {
            filter: Some(Setting::Value(fields)),
            ..
        })) = &kind
        {
            for name in fields.keys().filter(|n| filter::field(n).is_none()) {
                let names: Vec<&str> = filter::FIELDS.iter().map(|(n, ..)| *n).collect();
                problems.push(Problem::setting(
                    &id,
                    format!("filter.{name}"),
                    format!(
                        "the run filter has no field {name:?}; the fields are {}",
                        names.join(", ")
                    ),
                ));
            }
        }
        match kind {
            Some(kind) if problems.is_empty() => Ok(Node {
                id,
                input,
                inputs,
                position,
                kind,
            }),
            _ => Err(problems),
        }
    }
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let Value::Object(mut map) = serde_json::to_value(&self.kind).map_err(S::Error::custom)?
        else {
            return Err(S::Error::custom("node settings are not an object"));
        };
        map.insert("id".into(), Value::String(self.id.clone()));
        if let Some(input) = &self.input {
            map.insert("input".into(), Value::String(input.clone()));
        }
        if let Some(inputs) = &self.inputs {
            map.insert(
                "inputs".into(),
                serde_json::to_value(inputs).map_err(S::Error::custom)?,
            );
        }
        if let Some(position) = &self.position {
            map.insert(
                "position".into(),
                serde_json::to_value(position).map_err(S::Error::custom)?,
            );
        }
        map.serialize(s)
    }
}

const SPEC_KEYS: [&str; 3] = ["format", "variables", "nodes"];

impl Spec {
    /// Reads a spec. Unknown node types and settings are problems of their
    /// node and setting; the other nodes are still read and checked.
    pub fn from_json(value: &Value) -> Result<Spec, Vec<Problem>> {
        let Value::Object(top) = value else {
            return Err(vec![Problem::spec("a chart spec is a JSON object")]);
        };
        let mut problems = Vec::new();
        for key in top.keys().filter(|k| !SPEC_KEYS.contains(&k.as_str())) {
            problems.push(Problem::spec_setting(
                key,
                format!("unknown setting {key:?}; a spec has format, variables and nodes"),
            ));
        }
        let format = match top.get("format") {
            Some(v) if v.as_u64() == Some(u64::from(FORMAT)) => FORMAT,
            Some(v) => {
                problems.push(Problem::spec_setting(
                    "format",
                    format!("unsupported format {v}; this server reads format {FORMAT}"),
                ));
                0
            }
            None => {
                problems.push(Problem::spec_setting(
                    "format",
                    format!("a spec needs its format: {FORMAT}"),
                ));
                0
            }
        };
        let mut variables = Vec::new();
        match top.get("variables") {
            None => {}
            Some(Value::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    match Variable::deserialize(item) {
                        Ok(v) => variables.push(v),
                        Err(e) => problems.push(Problem::spec_setting(
                            format!("variables[{i}]"),
                            e.to_string(),
                        )),
                    }
                }
            }
            Some(_) => problems.push(Problem::spec_setting("variables", "expected a list")),
        }
        let mut nodes = Vec::new();
        match top.get("nodes") {
            Some(Value::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    match Node::from_json(i, item) {
                        Ok(n) => nodes.push(n),
                        Err(p) => problems.extend(p),
                    }
                }
            }
            Some(_) => problems.push(Problem::spec_setting("nodes", "expected a list")),
            None => problems.push(Problem::spec_setting("nodes", "a spec needs its nodes")),
        }
        if problems.is_empty() {
            Ok(Spec {
                format,
                variables,
                nodes,
            })
        } else {
            Err(problems)
        }
    }

    /// The spec as JSON.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("a spec serializes")
    }

    /// The node `id`.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The variable `name`.
    pub fn variable(&self, name: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.name == name)
    }
}

impl<'de> Deserialize<'de> for Spec {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        Spec::from_json(&value).map_err(|problems| {
            let messages: Vec<String> = problems.iter().map(ToString::to_string).collect();
            D::Error::custom(messages.join("; "))
        })
    }
}

/// True if `name` is a plain identifier, as the DSL reads names: a letter
/// or `_`, then letters, digits and `_`.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn push(out: &mut Vec<(String, String)>, setting: &str, message: String) {
    out.push((setting.to_owned(), message));
}

fn check_functions(
    out: &mut Vec<(String, String)>,
    functions: &[Function],
    allowed: &[FunctionName],
    node: &str,
) {
    for (i, f) in functions.iter().enumerate() {
        let at = format!("functions[{i}]");
        if !allowed.contains(&f.name) {
            let names: Vec<&str> = allowed.iter().map(|f| f.as_str()).collect();
            out.push((
                at.clone(),
                format!(
                    "{} is not a function of {node}; the functions are {}",
                    f.name.as_str(),
                    names.join(", ")
                ),
            ));
        }
        match (f.name, f.p) {
            (FunctionName::Percentile, None) => {
                out.push((at, "percentile needs p, from 0 to 100".into()));
            }
            (FunctionName::Percentile, Some(p)) if !(0.0..=100.0).contains(&p) => {
                out.push((at, format!("p is {p}; it must be from 0 to 100")));
            }
            (FunctionName::Percentile, Some(_)) => {}
            (name, Some(_)) => out.push((at, format!("{} takes no p", name.as_str()))),
            (_, None) => {}
        }
    }
}

impl NodeKind {
    /// Checks the literal values of the settings: ranges, and settings that
    /// need or exclude each other. Variables are checked once they are
    /// bound. Returns the setting and the message of each problem.
    pub fn check_values(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        match self {
            NodeKind::Filter(n) => {
                for (i, c) in n.conditions.iter().enumerate() {
                    let at = format!("conditions[{i}].value");
                    let value = c.value.as_ref();
                    match (c.op, value) {
                        (Op::Exists, Some(_)) => {
                            push(&mut out, &at, "exists takes no value".into())
                        }
                        (Op::Exists, None) => {}
                        (_, None) => push(&mut out, &at, "the condition needs a value".into()),
                        (_, Some(Setting::Var(_))) => {}
                        (Op::In | Op::NotIn, Some(Setting::Value(v))) => {
                            if !v.is_array() {
                                push(&mut out, &at, "in and not in take a list of values".into());
                            }
                        }
                        (Op::Lt | Op::Le | Op::Gt | Op::Ge, Some(Setting::Value(v))) => {
                            let time = v
                                .as_str()
                                .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok());
                            if !v.is_number() && !time {
                                push(
                                    &mut out,
                                    &at,
                                    "a comparison takes a number or a time".into(),
                                );
                            }
                        }
                        (Op::Eq | Op::Ne, Some(Setting::Value(v))) => {
                            if !(v.is_string() || v.is_number() || v.is_boolean()) {
                                push(
                                    &mut out,
                                    &at,
                                    "= and != take a text, number or boolean".into(),
                                );
                            }
                        }
                    }
                }
            }
            NodeKind::Derive(n) => {
                if !is_identifier(&n.name) {
                    push(
                        &mut out,
                        "name",
                        format!(
                            "{:?} is not a name: use letters, digits and _, starting with a letter",
                            n.name
                        ),
                    );
                }
                if n.expression.trim().is_empty() {
                    push(&mut out, "expression", "the expression is empty".into());
                }
            }
            NodeKind::Group(n) => {
                if let Setting::Value(by) = &n.by
                    && by.is_empty()
                {
                    push(&mut out, "by", "group needs at least one attribute".into());
                }
            }
            NodeKind::Flatten(n) => {
                if n.levels == Some(FlattenLevels::Count(0)) {
                    push(
                        &mut out,
                        "levels",
                        "flatten removes at least one level".into(),
                    );
                }
            }
            NodeKind::Sort(n) => {
                if n.by.is_empty() {
                    push(&mut out, "by", "sort needs at least one key".into());
                }
            }
            NodeKind::SelectLatest(n) => {
                if let Some(Setting::Value(0)) = n.runs {
                    push(&mut out, "runs", "select at least one run".into());
                }
            }
            NodeKind::Resample(n) => {
                if let Some(Setting::Value(p)) = n.points
                    && !(2..=MAX_GRID_POINTS).contains(&p)
                {
                    push(
                        &mut out,
                        "points",
                        format!("a grid has 2 to {MAX_GRID_POINTS} points, not {p}"),
                    );
                }
                if let (Some(Setting::Value(from)), Some(Setting::Value(to))) = (&n.from, &n.to)
                    && from >= to
                {
                    push(
                        &mut out,
                        "to",
                        format!("to ({to}) must be greater than from ({from})"),
                    );
                }
            }
            NodeKind::Match(n) => {
                if n.except.is_some() && n.only.is_some() {
                    push(&mut out, "only", "use except or only, not both".into());
                }
            }
            NodeKind::Reduce(n) => {
                check_functions(&mut out, &n.functions, FunctionName::REDUCE, "reduce");
                if let Some(Setting::Value(c)) = n.confidence
                    && !(c > 0.0 && c < 1.0)
                {
                    out.push((
                        "confidence".into(),
                        format!("confidence is {c}; it must be between 0 and 1"),
                    ));
                }
            }
            NodeKind::Combine(n) => match (&n.functions, &n.expressions) {
                (Some(_), Some(_)) => {
                    push(
                        &mut out,
                        "expressions",
                        "use functions or expressions, not both".into(),
                    );
                }
                (None, None) => {
                    push(
                        &mut out,
                        "functions",
                        "combine needs functions or expressions".into(),
                    );
                }
                (Some(functions), None) => {
                    if functions.is_empty() {
                        push(
                            &mut out,
                            "functions",
                            "combine needs at least one function".into(),
                        );
                    }
                    check_functions(&mut out, functions, FunctionName::COMBINE, "combine");
                }
                (None, Some(expressions)) => {
                    if n.column.is_some() {
                        push(
                            &mut out,
                            "column",
                            "column is a setting of functions".into(),
                        );
                    }
                    for (i, e) in expressions.iter().enumerate() {
                        if !is_identifier(&e.name) {
                            push(
                                &mut out,
                                &format!("expressions[{i}].name"),
                                format!(
                                    "{:?} is not a name: use letters, digits and _, starting with a letter",
                                    e.name
                                ),
                            );
                        } else if expressions[..i].iter().any(|o| o.name == e.name) {
                            push(
                                &mut out,
                                &format!("expressions[{i}].name"),
                                format!("another expression is called {:?}", e.name),
                            );
                        }
                        if e.expression.trim().is_empty() {
                            push(
                                &mut out,
                                &format!("expressions[{i}].expression"),
                                "the expression is empty".into(),
                            );
                        }
                    }
                }
            },
            NodeKind::Table(c) => {
                if let Setting::Value(columns) = &c.columns
                    && columns.is_empty()
                {
                    push(
                        &mut out,
                        "columns",
                        "a table needs at least one column".into(),
                    );
                }
            }
            NodeKind::Source(_)
            | NodeKind::Concatenate(_)
            | NodeKind::Line(_)
            | NodeKind::Scatter(_)
            | NodeKind::Bar(_)
            | NodeKind::Box(_)
            | NodeKind::Heatmap(_)
            | NodeKind::Value(_) => {}
        }
        if let Some(viz) = self.visualization()
            && let Some(levels) = viz.levels
        {
            for (i, l) in levels.iter().enumerate() {
                if levels[..i].contains(l) {
                    out.push((
                        format!("levels[{i}]"),
                        "each role takes one group level".into(),
                    ));
                }
            }
        }
        out
    }
}
