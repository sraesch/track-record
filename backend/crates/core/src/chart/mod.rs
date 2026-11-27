//! Charts (FR-22, FR-25, ADR 0022): the data that flows through the graph
//! of a chart, the JSON spec of a chart and its checks.
//!
//! - [`frame`]: frames, points and groups.
//! - [`attributes`]: the attributes frames and points carry, and keys.
//! - [`spec`]: the spec, format 1, read with [`Spec::from_json`].
//! - [`graph`]: the checks that make a spec **well-formed** ([`Graph::new`]):
//!   ids, inputs, no cycle, one visualization, limits. Only a well-formed
//!   spec can be saved.
//! - [`infer`]: the checks that make it **valid** ([`validate`]): variables,
//!   values, expressions, attributes, and the columns, units and group
//!   levels of each node. A spec with problems can be saved but not run.
//! - [`variables`]: variables, and binding their values ([`Spec::bind`]).
//! - [`eval`]: what the nodes compute ([`eval::run`]), on the trees that
//!   the server loads for the sources.
//!
//! Like the rest of this crate, nothing here does I/O: the units of the
//! metrics a source reads come through [`crate::dsl::check::Env`], and the
//! frames of the sources come from the server.

pub mod attributes;
pub mod eval;
pub mod filter;
pub mod frame;
pub mod graph;
pub mod infer;
pub mod problem;
pub mod spec;
pub mod variables;

#[cfg(test)]
mod tests;

pub use frame::{Attr, AttrValue, Attributes, Column, Frame, Group, Tree};
pub use graph::Graph;
pub use infer::{ColumnShape, Level, Shape, Validation, validate};
pub use problem::Problem;
pub use spec::{Node, NodeKind, Spec};
pub use variables::{Setting, VarType, Variable};

use serde::Serialize;
use serde_json::Value;

use crate::dsl::check::Env;
use crate::units::UnitCatalogue;

/// The version of the spec format this code reads and writes.
pub const FORMAT: u32 = 1;
/// Most nodes in a graph.
pub const MAX_NODES: usize = 100;
/// Most sources in a graph.
pub const MAX_SOURCES: usize = 8;

/// The result of checking a spec in JSON.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The spec can be saved.
    pub well_formed: bool,
    /// The spec can be run: well-formed and without problems.
    pub valid: bool,
    pub problems: Vec<Problem>,
    /// The shape of the output of each node whose inputs could be checked.
    pub nodes: std::collections::BTreeMap<String, Shape>,
}

/// Reads and checks a spec: well-formed, then valid.
pub fn check(value: &Value, env: &mut dyn Env, catalogue: &UnitCatalogue) -> Report {
    let spec = match Spec::from_json(value) {
        Ok(spec) => spec,
        Err(problems) => return Report::malformed(problems),
    };
    let graph = match Graph::new(&spec) {
        Ok(graph) => graph,
        Err(problems) => return Report::malformed(problems),
    };
    let v = validate(&spec, &graph, env, catalogue);
    Report {
        well_formed: true,
        valid: v.problems.is_empty(),
        problems: v.problems,
        nodes: v.shapes,
    }
}

impl Report {
    fn malformed(problems: Vec<Problem>) -> Self {
        Self {
            well_formed: false,
            valid: false,
            problems,
            nodes: Default::default(),
        }
    }
}
