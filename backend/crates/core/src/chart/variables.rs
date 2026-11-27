//! Variables of a chart (FR-25, ADR 0022, "Variables").
//!
//! A chart declares variables with a type and an optional default. A value
//! setting of a node, or one field of a run filter, can be
//! `{"var": "<name>"}` instead of a literal ([`Setting`]). The type of the
//! variable must be the type of the setting ([`Expect`]). A variable
//! replaces a value; it cannot add or remove nodes.

use std::collections::BTreeMap;
use std::fmt;

use chrono::DateTime;
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::filter;
use super::problem::Problem;
use super::spec::Spec;

/// The type of a variable, and of the settings it can stand for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    Text,
    Number,
    Boolean,
    /// A time in RFC 3339, e.g. `2026-11-27T00:00:00Z`.
    Time,
    /// A metric expression (FR-17a).
    Metric,
    /// A run filter (FR-11): an object with the fields of a source's filter.
    RunFilter,
    /// The name of an attribute or a column.
    Attribute,
}

impl VarType {
    pub fn as_str(self) -> &'static str {
        match self {
            VarType::Text => "text",
            VarType::Number => "number",
            VarType::Boolean => "boolean",
            VarType::Time => "time",
            VarType::Metric => "metric",
            VarType::RunFilter => "run_filter",
            VarType::Attribute => "attribute",
        }
    }
}

/// What a setting takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// One value of the type, or a list of them.
    Type { ty: VarType, list: bool },
    /// The value of a filter condition: a text, number, boolean or time, or
    /// a list of them.
    Value,
    /// A field of a run filter: the type the field table gives, and the
    /// values the server's run filter takes ([`filter::check`]).
    Field(&'static str),
}

impl Expect {
    pub const fn one(ty: VarType) -> Self {
        Expect::Type { ty, list: false }
    }

    pub const fn list(ty: VarType) -> Self {
        Expect::Type { ty, list: true }
    }

    /// True if a variable of type `ty` (a list if `list`) can stand for a
    /// setting that takes this.
    pub fn accepts(self, ty: VarType, list: bool) -> bool {
        match self {
            Expect::Field(name) => filter::type_of(name).accepts(ty, list),
            Expect::Type { ty: t, list: l } => t == ty && l == list,
            Expect::Value => matches!(
                ty,
                VarType::Text | VarType::Number | VarType::Boolean | VarType::Time
            ),
        }
    }
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expect::Type { ty, list: false } => f.write_str(ty.as_str()),
            Expect::Type { ty, list: true } => write!(f, "a list of {}", ty.as_str()),
            Expect::Value => f.write_str("a text, number, boolean or time"),
            Expect::Field(name) => filter::type_of(name).fmt(f),
        }
    }
}

/// Checks that a JSON value fits what a setting takes.
pub fn fits(expect: Expect, value: &Value) -> Result<(), String> {
    match expect {
        Expect::Type { ty, list: true } => match value {
            Value::Array(items) => items
                .iter()
                .enumerate()
                .try_for_each(|(i, v)| fits_one(ty, v).map_err(|e| format!("item {i}: {e}"))),
            _ => Err(format!("expected a list of {}", ty.as_str())),
        },
        Expect::Type { ty, list: false } => fits_one(ty, value),
        Expect::Value => match value {
            Value::Array(items) => items.iter().try_for_each(fits_scalar),
            v => fits_scalar(v),
        },
        Expect::Field(name) => {
            fits(filter::type_of(name), value)?;
            filter::check(name, value)
        }
    }
}

fn fits_scalar(value: &Value) -> Result<(), String> {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => Ok(()),
        _ => Err("expected a text, number, boolean or time".into()),
    }
}

fn fits_one(ty: VarType, value: &Value) -> Result<(), String> {
    let ok = match (ty, value) {
        (VarType::Text, Value::String(_)) => true,
        (VarType::Number, Value::Number(_)) => true,
        (VarType::Boolean, Value::Bool(_)) => true,
        (VarType::Time, Value::String(s)) => {
            return DateTime::parse_from_rfc3339(s)
                .map(|_| ())
                .map_err(|_| format!("{s:?} is not a time in RFC 3339"));
        }
        (VarType::Metric | VarType::Attribute, Value::String(s)) => {
            if s.trim().is_empty() {
                return Err(format!("expected {}, got an empty text", ty.as_str()));
            }
            true
        }
        (VarType::RunFilter, Value::Object(fields)) => {
            for (name, v) in fields {
                let Some(expect) = filter::field(name) else {
                    return Err(format!("the run filter has no field {name:?}"));
                };
                fits(expect, v).map_err(|e| format!("filter field {name:?}: {e}"))?;
            }
            true
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!("expected {}", ty.as_str()))
    }
}

/// A variable a chart declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: VarType,
    /// A list of values of the type.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub list: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

impl Variable {
    pub fn expect(&self) -> Expect {
        Expect::Type {
            ty: self.ty,
            list: self.list,
        }
    }
}

/// True if `name` can name a variable: letters, digits, `_` and `-`, as it
/// is written in a URL (`?v.<name>=`).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A setting: a literal value, or a variable that stands for one.
#[derive(Debug, Clone, PartialEq)]
pub enum Setting<T> {
    Var(String),
    Value(T),
}

impl<T> Setting<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Setting::Value(v) => Some(v),
            Setting::Var(_) => None,
        }
    }

    pub fn var(&self) -> Option<&str> {
        match self {
            Setting::Var(name) => Some(name),
            Setting::Value(_) => None,
        }
    }
}

impl<T> From<T> for Setting<T> {
    fn from(value: T) -> Self {
        Setting::Value(value)
    }
}

/// The variable `{"var": "<name>"}` a JSON value stands for, if it is one.
fn as_var(value: &Value) -> Option<&str> {
    match value {
        Value::Object(m) if m.len() == 1 => m.get("var").and_then(Value::as_str),
        _ => None,
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Setting<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        if let Some(name) = as_var(&value) {
            return Ok(Setting::Var(name.to_owned()));
        }
        T::deserialize(value)
            .map(Setting::Value)
            .map_err(D::Error::custom)
    }
}

impl<T: Serialize> Serialize for Setting<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Setting::Var(name) => {
                use serde::ser::SerializeMap;
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("var", name)?;
                m.end()
            }
            Setting::Value(v) => v.serialize(s),
        }
    }
}

/// A setting as the checks see it, whatever its value type.
pub trait Slot {
    /// The variable the setting stands for.
    fn var(&self) -> Option<&str>;
    /// The literal value as JSON.
    fn literal(&self) -> Option<Value>;
    /// Replaces the setting by `value`.
    fn bind(&mut self, value: &Value) -> Result<(), String>;
}

impl<T: Serialize + DeserializeOwned> Slot for Setting<T> {
    fn var(&self) -> Option<&str> {
        Setting::var(self)
    }

    fn literal(&self) -> Option<Value> {
        self.value().and_then(|v| serde_json::to_value(v).ok())
    }

    fn bind(&mut self, value: &Value) -> Result<(), String> {
        let v = T::deserialize(value.clone()).map_err(|e| e.to_string())?;
        *self = Setting::Value(v);
        Ok(())
    }
}

/// Called for every setting that can stand for a variable: its path in the
/// node, what it takes and the setting.
pub type Visit<'a> = dyn FnMut(&str, Expect, &mut dyn Slot) + 'a;

/// Checks the variables of a spec and the settings that use them: names,
/// defaults, that every variable a setting names exists and has the
/// setting's type, and the literal values of run filter fields and filter
/// conditions.
pub fn check_variables(spec: &Spec) -> Vec<Problem> {
    let mut problems = Vec::new();
    for (i, v) in spec.variables.iter().enumerate() {
        if !valid_name(&v.name) {
            problems.push(Problem::spec_setting(
                format!("variables[{i}].name"),
                format!(
                    "{:?} is not a variable name: use letters, digits, _ and -",
                    v.name
                ),
            ));
        } else if spec.variables[..i].iter().any(|o| o.name == v.name) {
            problems.push(Problem::spec_setting(
                format!("variables[{i}].name"),
                format!("another variable is called {:?}", v.name),
            ));
        }
        if let Some(default) = &v.default
            && let Err(e) = fits(v.expect(), default)
        {
            problems.push(Problem::spec_setting(
                format!("variables[{i}].default"),
                format!("the default does not fit the type of the variable: {e}"),
            ));
        }
    }
    for node in &spec.nodes {
        let mut kind = node.kind.clone();
        kind.visit(&mut |path, expect, slot| {
            if let Some(name) = slot.var().map(str::to_owned) {
                match spec.variable(&name) {
                    None => problems.push(Problem::setting(
                        &node.id,
                        path,
                        format!("there is no variable {name:?}"),
                    )),
                    Some(v) if !expect.accepts(v.ty, v.list) => {
                        problems.push(Problem::setting(&node.id, path, mismatch(v, expect)));
                    }
                    // The type of a variable is wider than some settings
                    // (`number` for a count): the default must fit this one.
                    Some(Variable {
                        default: Some(default),
                        ..
                    }) => {
                        let default = default.clone();
                        if let Err(e) = fits(expect, &default).and_then(|()| slot.bind(&default)) {
                            problems.push(Problem::setting(
                                &node.id,
                                path,
                                format!("the default of variable {name:?} does not fit: {e}"),
                            ));
                        }
                    }
                    Some(_) => {}
                }
            } else if matches!(expect, Expect::Value | Expect::Field(_))
                && let Some(value) = slot.literal()
                && let Err(e) = fits(expect, &value)
            {
                problems.push(Problem::setting(&node.id, path, e));
            }
        });
    }
    problems
}

fn mismatch(v: &Variable, expect: Expect) -> String {
    let ty = if v.list {
        format!("a list of {}", v.ty.as_str())
    } else {
        v.ty.as_str().to_owned()
    };
    format!(
        "variable {:?} is {ty}, but the setting takes {expect}",
        v.name
    )
}

impl Spec {
    /// The spec with every variable replaced by its value in `values`, or
    /// else by its default. A variable without either, a value of the
    /// wrong type and a value for a variable the spec does not declare are
    /// problems. The literal values are checked again after binding.
    pub fn bind(&self, values: &BTreeMap<String, Value>) -> Result<Spec, Vec<Problem>> {
        let mut problems = Vec::new();
        for name in values.keys().filter(|n| self.variable(n).is_none()) {
            problems.push(Problem::spec_setting(
                format!("variables.{name}"),
                format!("the chart has no variable {name:?}"),
            ));
        }
        let mut resolved = BTreeMap::new();
        for v in &self.variables {
            match values.get(&v.name).or(v.default.as_ref()) {
                Some(value) => match fits(v.expect(), value) {
                    Ok(()) => {
                        resolved.insert(v.name.as_str(), value);
                    }
                    Err(e) => problems.push(Problem::spec_setting(
                        format!("variables.{}", v.name),
                        format!("the value of variable {:?} does not fit: {e}", v.name),
                    )),
                },
                None => problems.push(Problem::spec_setting(
                    format!("variables.{}", v.name),
                    format!("variable {:?} has no value and no default", v.name),
                )),
            }
        }
        let mut spec = self.clone();
        for node in &mut spec.nodes {
            let id = node.id.clone();
            node.kind.visit(&mut |path, expect, slot| {
                let Some(name) = slot.var().map(str::to_owned) else {
                    return;
                };
                let Some(v) = self.variable(&name) else {
                    problems.push(Problem::setting(
                        &id,
                        path,
                        format!("there is no variable {name:?}"),
                    ));
                    return;
                };
                if !expect.accepts(v.ty, v.list) {
                    problems.push(Problem::setting(&id, path, mismatch(v, expect)));
                    return;
                }
                // A variable without a fitting value was reported above.
                if let Some(value) = resolved.get(name.as_str())
                    && let Err(e) = fits(expect, value).and_then(|()| slot.bind(value))
                {
                    problems.push(Problem::setting(
                        &id,
                        path,
                        format!("variable {name:?}: {e}"),
                    ));
                }
            });
        }
        for node in &spec.nodes {
            for (setting, message) in node.kind.check_values() {
                problems.push(Problem::setting(&node.id, setting, message));
            }
        }
        if problems.is_empty() {
            Ok(spec)
        } else {
            Err(problems)
        }
    }

    /// The spec with the variables that have a fitting default replaced by
    /// it; the others stay. Validation infers columns from this.
    pub fn bind_defaults(&self) -> Spec {
        let mut spec = self.clone();
        for node in &mut spec.nodes {
            node.kind.visit(&mut |_, expect, slot| {
                let Some(v) = slot.var().and_then(|name| self.variable(name)) else {
                    return;
                };
                if let Some(default) = &v.default
                    && expect.accepts(v.ty, v.list)
                    && fits(expect, default).is_ok()
                {
                    let _ = slot.bind(default);
                }
            });
        }
        spec
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_setting_is_a_variable_or_a_literal() {
        let s: Setting<u32> = serde_json::from_value(json!({"var": "n"})).unwrap();
        assert_eq!(s, Setting::Var("n".into()));
        let s: Setting<u32> = serde_json::from_value(json!(5)).unwrap();
        assert_eq!(s, Setting::Value(5));
        let e = serde_json::from_value::<Setting<u32>>(json!("five")).unwrap_err();
        assert!(e.to_string().contains("expected u32"), "{e}");
        // An object with more than `var` is a literal.
        let s: Setting<Value> = serde_json::from_value(json!({"var": "n", "x": 1})).unwrap();
        assert_eq!(s, Setting::Value(json!({"var": "n", "x": 1})));
        for v in [json!({"var": "n"}), json!(5)] {
            let s: Setting<u32> = serde_json::from_value(v.clone()).unwrap();
            assert_eq!(serde_json::to_value(&s).unwrap(), v);
        }
    }

    #[test]
    fn values_fit_their_type() {
        assert!(fits(Expect::one(VarType::Text), &json!("main")).is_ok());
        assert!(fits(Expect::one(VarType::Text), &json!(1)).is_err());
        assert!(fits(Expect::one(VarType::Number), &json!(1.5)).is_ok());
        assert!(fits(Expect::one(VarType::Boolean), &json!("true")).is_err());
        assert!(fits(Expect::one(VarType::Time), &json!("2026-11-27T10:00:00Z")).is_ok());
        assert!(fits(Expect::one(VarType::Time), &json!("yesterday")).is_err());
        assert!(fits(Expect::one(VarType::Metric), &json!(" ")).is_err());
        assert!(fits(Expect::list(VarType::Attribute), &json!(["case", "run.id"])).is_ok());
        assert!(fits(Expect::list(VarType::Attribute), &json!("case")).is_err());
        assert!(fits(Expect::list(VarType::Attribute), &json!(["case", 1])).is_err());
        assert!(fits(Expect::Value, &json!([1, "a", true])).is_ok());
        assert!(fits(Expect::Value, &json!(null)).is_err());
        assert!(
            fits(
                Expect::one(VarType::RunFilter),
                &json!({"sut_branch": "main"})
            )
            .is_ok()
        );
        let e = fits(Expect::one(VarType::RunFilter), &json!({"branch": "main"})).unwrap_err();
        assert!(e.contains("no field \"branch\""), "{e}");
        let e = fits(
            Expect::one(VarType::RunFilter),
            &json!({"latest_runs": "5"}),
        )
        .unwrap_err();
        assert!(e.contains("latest_runs"), "{e}");
    }

    #[test]
    fn variables_fit_settings_of_their_type() {
        assert!(Expect::one(VarType::Text).accepts(VarType::Text, false));
        assert!(!Expect::one(VarType::Text).accepts(VarType::Metric, false));
        assert!(!Expect::list(VarType::Text).accepts(VarType::Text, false));
        assert!(Expect::Value.accepts(VarType::Number, true));
        assert!(!Expect::Value.accepts(VarType::RunFilter, false));
        assert!(valid_name("sut-branch_2"));
        assert!(!valid_name("a b"));
        assert!(!valid_name(""));
    }
}
