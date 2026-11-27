//! The derive node (ADR 0022, "Nodes"): an attribute computed from numeric
//! attributes, `attr.<name>`.
//!
//! The expression uses the DSL's grammar, operators, scalar functions and
//! unit rules ([`crate::dsl::parse_attributes`]). The units of attributes
//! come from the data (a parameter has the unit its test case gives it), so
//! the expression is checked for each combination of units it meets. A
//! frame, or a point, that lacks an attribute of the expression, has one
//! that is not a number, has units that do not fit the expression, or for
//! which the expression has no value, does not get the new attribute.

use std::collections::HashMap;

use super::{Result, map_frames};
use crate::chart::frame::{Attr, AttrValue, Tree};
use crate::chart::problem::Problem;
use crate::chart::spec::DeriveNode;
use crate::dsl::check::{Checked, Env, SeriesUnits, Signature, check};
use crate::dsl::eval::{EvalError, Inputs, Value, eval};
use crate::dsl::{Expr, parse_attributes};
use crate::units::UnitCatalogue;

/// Adds `attr.<name>` to every frame whose attributes the expression can
/// read, and to every point that has attributes of its own for it.
pub fn derive(id: &str, n: &DeriveNode, tree: Tree, catalogue: &UnitCatalogue) -> Result<Tree> {
    let expr =
        parse_attributes(&n.expression).map_err(|d| Problem::expression(id, "expression", &d))?;
    let mut names: Vec<String> = expr.attributes().into_iter().map(|(n, _)| n).collect();
    names.sort();
    names.dedup();
    let mut d = Deriver {
        expr: &expr,
        names,
        declared: n.unit.as_deref(),
        catalogue,
        checked: HashMap::new(),
    };
    let name = format!("attr.{}", n.name);
    map_frames(tree, &mut |mut frames| {
        for f in &mut frames {
            if let Some(attr) = d.value(|a| f.attrs.get(a)) {
                f.attrs.insert(name.clone(), attr);
            }
            // Points with attributes of their own for the expression can
            // have a value of their own.
            let Some(points) = &f.point_attrs else {
                continue;
            };
            let own: Vec<Option<Attr>> = points
                .iter()
                .map(|p| {
                    if !d.names.iter().any(|a| p.contains_key(a)) {
                        return None;
                    }
                    d.value(|a| p.get(a).or_else(|| f.attrs.get(a)))
                })
                .collect();
            if let Some(points) = &mut f.point_attrs {
                for (p, attr) in points.iter_mut().zip(own) {
                    if let Some(attr) = attr {
                        p.insert(name.clone(), attr);
                    }
                }
            }
        }
        Ok(frames)
    })
}

struct Deriver<'a> {
    expr: &'a Expr,
    /// The attributes the expression reads.
    names: Vec<String>,
    declared: Option<&'a str>,
    catalogue: &'a UnitCatalogue,
    /// The checked expression for each combination of the units of its
    /// attributes; `None` if they do not fit it.
    checked: HashMap<Vec<String>, Option<Checked>>,
}

impl Deriver<'_> {
    /// The new attribute for the attributes `get` gives, if it has a value.
    fn value<'v>(&mut self, get: impl Fn(&str) -> Option<&'v Attr>) -> Option<Attr> {
        let mut values = Vec::with_capacity(self.names.len());
        let mut units = Vec::with_capacity(self.names.len());
        for name in &self.names {
            let attr = get(name)?;
            values.push(attr.value.as_f64()?);
            units.push(attr.unit.clone());
        }
        let checked = self
            .checked
            .entry(units)
            .or_insert_with_key(|units| {
                let mut env = AttrEnv {
                    names: &self.names,
                    units,
                };
                check(self.expr, &mut env, self.catalogue, self.declared).ok()
            })
            .as_ref()?;
        let mut inputs = AttrInputs {
            names: &self.names,
            values: &values,
        };
        let value = eval(&checked.node, &mut inputs).ok()?;
        let Value::Scalar(Some(v)) = value.scale(checked.output_factor) else {
            return None;
        };
        Some(Attr::with_unit(
            AttrValue::number(v)?,
            checked.signature.unit.clone(),
        ))
    }
}

/// The units of the attributes of one frame.
struct AttrEnv<'a> {
    names: &'a [String],
    units: &'a [String],
}

impl Env for AttrEnv<'_> {
    fn metric(&self, _: &str) -> Option<String> {
        None
    }

    fn series(&self, _: &str) -> Option<SeriesUnits> {
        None
    }

    fn derived(&mut self, name: &str) -> std::result::Result<Signature, String> {
        Err(format!("derived.{name} is a metric, not an attribute"))
    }

    fn attribute(&mut self, name: &str) -> std::result::Result<String, String> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| self.units[i].clone())
            .ok_or_else(|| format!("unknown attribute {name}"))
    }
}

/// The values of the attributes of one frame or point.
struct AttrInputs<'a> {
    names: &'a [String],
    values: &'a [f64],
}

impl Inputs for AttrInputs<'_> {
    fn metric(&mut self, name: &str) -> std::result::Result<f64, EvalError> {
        Err(EvalError::Missing(name.to_owned()))
    }

    fn series(&mut self, name: &str) -> std::result::Result<crate::series::Series, EvalError> {
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
