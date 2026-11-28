//! The requests and responses of the chart endpoints (ADR 0022,
//! "Evaluation", "Interfaces"). The types of `track_record_core::chart`
//! stay free of the API; these mirror them for JSON and OpenAPI.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use track_record_core::chart::{self, Attr, AttrValue, Attributes, Frame, Tree};
use utoipa::openapi::schema::{AdditionalProperties, AnyOfBuilder, ObjectBuilder, Schema};
use utoipa::openapi::{Ref, RefOr};
use utoipa::{PartialSchema, ToSchema};

use crate::service::analysis::Excluded;
use crate::service::table::ExportFormat;

/// Runs a chart spec that is not saved (FR-22, FR-23).
#[derive(Debug, Clone, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunChart {
    /// The spec (format 1, ADR 0022).
    #[schema(value_type = HashMap<String, serde_json::Value>)]
    pub spec: Value,
    /// Values of the chart's variables, by name. A variable without a value
    /// takes its default; without one, the run is rejected.
    #[serde(default)]
    pub variables: BTreeMap<String, Value>,
    /// Stops at this node and returns its output instead of what reaches
    /// the visualization (preview of a node).
    #[serde(default)]
    pub until: Option<String>,
    /// Limits each source to the latest run that matches its filter. The
    /// result is marked `partial`.
    #[serde(default)]
    pub preview: bool,
    /// Most points per line for display (at least 2, default 2000): each
    /// frame that reaches a `line` keeps the minimum and maximum of each
    /// bucket of its points. Other visualizations are not downsampled.
    #[serde(default)]
    pub max_points: Option<usize>,
    /// Streams the data that reaches the visualization as a file instead of
    /// events (FR-20): one row per point with its attributes and columns.
    #[serde(default)]
    pub format: Option<ExportFormat>,
}

/// Checks a chart spec without running it.
#[derive(Debug, Clone, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateChart {
    /// The spec (format 1, ADR 0022). Settings that are variables are
    /// checked with their defaults.
    #[schema(value_type = HashMap<String, serde_json::Value>)]
    pub spec: Value,
}

/// The problems of a spec and the output of each node.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartValidation {
    /// The spec can be saved: known node types and settings, unique ids,
    /// inputs that exist, no cycle, one visualization, within the limits.
    pub well_formed: bool,
    /// The spec can be run: well-formed and without problems.
    pub valid: bool,
    pub problems: Vec<ChartProblem>,
    /// The output of each node whose inputs could be checked, by node id.
    pub nodes: BTreeMap<String, NodeShape>,
}

/// One problem of a spec or of a run.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartProblem {
    /// Id of the node; none for a problem of the whole spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Path of the setting, e.g. `filter.sut_branch`, `functions[1]` or
    /// `variables[0].default`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<String>,
    pub message: String,
    /// Byte offsets of the part of the setting's expression, as the DSL
    /// reports them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
}

impl From<&chart::Problem> for ChartProblem {
    fn from(p: &chart::Problem) -> Self {
        Self {
            node: p.node.clone(),
            setting: p.setting.clone(),
            message: p.message.clone(),
            start: p.span.map(|s| s.start),
            end: p.span.map(|s| s.end),
        }
    }
}

/// The output of a node as validation infers it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NodeShape {
    /// Group levels, outermost first.
    pub levels: Vec<LevelShape>,
    /// Columns of the frames; of the frames of `a` below a match level.
    pub columns: Vec<ColumnShape>,
    /// Attributes frames and points can have. `param.*` and `asset.*`
    /// stand for every parameter and asset.
    pub attributes: BTreeSet<String>,
}

/// A group level: made by a group node (`by`, `except`) or by a match
/// (`inputs`).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LevelShape {
    Group {
        by: Vec<String>,
        except: Vec<String>,
    },
    Match {
        inputs: Vec<MatchInputShape>,
    },
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MatchInputShape {
    pub name: String,
    pub columns: Vec<ColumnShape>,
    pub attributes: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ColumnShape {
    pub name: String,
    /// `""` for no unit; null if the unit is known only when the chart
    /// runs.
    pub unit: Option<String>,
}

fn columns(c: &[chart::ColumnShape]) -> Vec<ColumnShape> {
    c.iter()
        .map(|c| ColumnShape {
            name: c.name.clone(),
            unit: c.unit.clone(),
        })
        .collect()
}

impl From<&chart::Shape> for NodeShape {
    fn from(s: &chart::Shape) -> Self {
        Self {
            levels: s
                .levels
                .iter()
                .map(|l| match l {
                    chart::Level::Group { by, except } => LevelShape::Group {
                        by: by.clone(),
                        except: except.clone(),
                    },
                    chart::Level::Match { inputs } => LevelShape::Match {
                        inputs: inputs
                            .iter()
                            .map(|i| MatchInputShape {
                                name: i.name.clone(),
                                columns: columns(&i.columns),
                                attributes: i.attributes.clone(),
                            })
                            .collect(),
                    },
                })
                .collect(),
            columns: columns(&s.columns),
            attributes: s.attributes.clone(),
        }
    }
}

impl From<chart::Report> for ChartValidation {
    fn from(r: chart::Report) -> Self {
        Self {
            well_formed: r.well_formed,
            valid: r.valid,
            problems: r.problems.iter().map(Into::into).collect(),
            nodes: r.nodes.iter().map(|(k, v)| (k.clone(), v.into())).collect(),
        }
    }
}

// Result tree --------------------------------------------------------------------

/// What a node returns: a list of frames, or of groups.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ChartTree {
    /// The groups, in order; absent when the tree is a list of frames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<ChartGroup>>,
    /// The frames, in order; absent when the tree is a list of groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<ChartFrame>>,
}

/// A group: the attributes it was formed by and its members.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartGroup {
    pub key: Vec<KeyValue>,
    #[schema(no_recursion)]
    pub tree: ChartTree,
}

/// An attribute of a group key; `value` is null for the subgroup of frames
/// without a value.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct KeyValue {
    pub name: String,
    pub value: Value,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub unit: String,
}

/// The value of an attribute and its unit (absent for none). Times are
/// RFC 3339 text; parameters keep their JSON type.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AttrJson {
    pub value: Value,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub unit: String,
}

/// A column of a frame.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartColumn {
    pub name: String,
    pub unit: String,
    /// The x-dimension of a series' `x` (`time`, `bytes`, ...).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dimension: String,
}

/// An ordered list of points with the same columns.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartFrame {
    pub attributes: BTreeMap<String, AttrJson>,
    pub columns: Vec<ChartColumn>,
    pub points: Vec<ChartPoint>,
    /// The input of a match the frame came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

/// A point: one value per column (null for no value), and the attributes
/// in which it differs from its frame.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartPoint {
    pub values: Vec<Option<f64>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, AttrJson>,
}

/// The JSON form of an attribute value.
pub fn attr_value(v: &AttrValue) -> Value {
    match v {
        AttrValue::Bool(b) => Value::Bool(*b),
        AttrValue::Number(n) => Value::Number(n.clone()),
        AttrValue::Text(t) => Value::String(t.clone()),
        AttrValue::Time(t) => Value::String(t.to_rfc3339()),
        AttrValue::List(items) => Value::Array(items.iter().map(attr_value).collect()),
    }
}

fn attr_json(a: &Attr) -> AttrJson {
    AttrJson {
        value: attr_value(&a.value),
        unit: a.unit.clone(),
    }
}

fn attributes(a: &Attributes) -> BTreeMap<String, AttrJson> {
    a.iter().map(|(k, v)| (k.clone(), attr_json(v))).collect()
}

impl From<&Frame> for ChartFrame {
    fn from(f: &Frame) -> Self {
        let points = (0..f.len())
            .map(|i| ChartPoint {
                values: f.values.iter().map(|c| c[i]).collect(),
                attributes: f
                    .point_attrs
                    .as_ref()
                    .and_then(|p| p.get(i))
                    .map(|own| {
                        own.iter()
                            .filter(|(k, v)| !f.attrs.get(*k).is_some_and(|a| a.same(v)))
                            .map(|(k, v)| (k.clone(), attr_json(v)))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();
        Self {
            attributes: attributes(&f.attrs),
            columns: f
                .columns
                .iter()
                .map(|c| ChartColumn {
                    name: c.name.clone(),
                    unit: c.unit.clone(),
                    dimension: c.dimension.clone(),
                })
                .collect(),
            points,
            input: f.input.clone(),
        }
    }
}

impl From<&Tree> for ChartTree {
    fn from(t: &Tree) -> Self {
        match t {
            Tree::Frames(frames) => Self {
                groups: None,
                frames: Some(frames.iter().map(Into::into).collect()),
            },
            Tree::Groups(groups) => Self {
                groups: Some(
                    groups
                        .iter()
                        .map(|g| ChartGroup {
                            key: g
                                .key
                                .iter()
                                .map(|(name, a)| KeyValue {
                                    name: name.clone(),
                                    value: a.as_ref().map_or(Value::Null, |a| attr_value(&a.value)),
                                    unit: a.as_ref().map(|a| a.unit.clone()).unwrap_or_default(),
                                })
                                .collect(),
                            tree: (&g.tree).into(),
                        })
                        .collect(),
                ),
                frames: None,
            },
        }
    }
}

// Events ----------------------------------------------------------------------------

/// One line of the NDJSON of a chart run: an event, or, with `format:
/// json`, a row of the file (attributes and columns by name). For the
/// OpenAPI document only: `anyOf`, since an event is an object too.
pub struct ChartLine;

impl utoipa::PartialSchema for ChartLine {
    fn schema() -> RefOr<Schema> {
        AnyOfBuilder::new()
            .item(Ref::from_schema_name(ChartEvent::name()))
            .item(
                ObjectBuilder::new()
                    .additional_properties(Some(AdditionalProperties::<Schema>::FreeForm(true))),
            )
            .description(Some(
                "A `ChartEvent`, or with `format: json` a row of the file: attributes and \
                 columns by name.",
            ))
            .into()
    }
}

impl ToSchema for ChartLine {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((ChartEvent::name().into(), ChartEvent::schema()));
        <ChartEvent as ToSchema>::schemas(schemas);
    }
}

/// The samples a source selected.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SourceSamples {
    pub id: String,
    pub samples: usize,
}

/// One line of the NDJSON stream of a chart run: `start`, `progress`, one
/// `result`, then `done` or `error`.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChartEvent {
    Start {
        /// The node whose output the result is.
        node: String,
        /// The result is not the whole chart: a preview, or a node other
        /// than the visualization.
        partial: bool,
        /// The sources the run reads, with the samples each selected.
        sources: Vec<SourceSamples>,
        total_samples: usize,
    },
    /// Samples of all sources loaded so far.
    Progress {
        done: usize,
        total: usize,
    },
    Result {
        node: String,
        partial: bool,
        tree: ChartTree,
        /// Samples left out per source because its metric does not apply
        /// to them.
        excluded: BTreeMap<String, Excluded>,
    },
    Done,
    /// The run failed after it started. `node` and `setting` say where,
    /// for an error of a node.
    Error {
        code: String,
        detail: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        node: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        setting: Option<String>,
    },
}
