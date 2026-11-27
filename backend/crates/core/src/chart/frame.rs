//! The data that flows through the graph of a chart (ADR 0022, "Frames,
//! points and groups").
//!
//! A frame is an ordered list of points with the same columns. Values are
//! stored per column, so that a column can go to [`crate::stats`] as it is.
//! A value can be no value (FR-12a); the point stays. Frames carry
//! attributes, and points can carry attributes of their own: where a point
//! has none for an attribute, the frame's value counts.
//!
//! The output of every node is a [`Tree`]: an ordered list of frames, or of
//! groups, never both. All frames are at the same depth.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::Value;

/// A column of a frame: its name and unit (`""` for none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub unit: String,
}

impl Column {
    pub fn new(name: impl Into<String>, unit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            unit: unit.into(),
        }
    }
}

/// The value of an attribute. Parameters keep their JSON type: `10` is not
/// `"10"`. Numbers stay JSON numbers, so that integers beyond the precision
/// of `f64` stay apart, and they compare as the analysis compares
/// parameters (`serde_json::Value` equality).
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    Bool(bool),
    Number(serde_json::Number),
    Text(String),
    Time(DateTime<Utc>),
    List(Vec<AttrValue>),
}

impl AttrValue {
    /// The value of a JSON scalar or list of scalars, e.g. a parameter.
    /// `None` for null, objects and lists that hold them.
    pub fn from_json(value: &Value) -> Option<AttrValue> {
        match value {
            Value::Bool(b) => Some(AttrValue::Bool(*b)),
            Value::Number(n) => Some(AttrValue::Number(n.clone())),
            Value::String(s) => Some(AttrValue::Text(s.clone())),
            Value::Array(items) => items
                .iter()
                .map(AttrValue::from_json)
                .collect::<Option<Vec<_>>>()
                .map(AttrValue::List),
            Value::Null | Value::Object(_) => None,
        }
    }

    /// A number computed by the graph, e.g. by a derive node. `None` for
    /// NaN and infinities, which are no value.
    pub fn number(value: f64) -> Option<AttrValue> {
        serde_json::Number::from_f64(value).map(AttrValue::Number)
    }

    /// The value as a number, for arithmetic and numeric order.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AttrValue::Number(n) => n.as_f64(),
            _ => None,
        }
    }
}

/// An attribute with its unit (`""` for none); parameters have one.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub value: AttrValue,
    pub unit: String,
}

impl Attr {
    pub fn new(value: AttrValue) -> Self {
        Self {
            value,
            unit: String::new(),
        }
    }

    pub fn with_unit(value: AttrValue, unit: impl Into<String>) -> Self {
        Self {
            value,
            unit: unit.into(),
        }
    }
}

/// Attributes by name.
pub type Attributes = BTreeMap<String, Attr>;

/// An ordered list of points with the same columns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub attrs: Attributes,
    pub columns: Vec<Column>,
    /// One list per column, in the order of `columns`, each with one value
    /// per point. `None` is no value.
    pub values: Vec<Vec<Option<f64>>>,
    /// Attributes of the points, one map per point, after a reduce or a
    /// concatenate. Where a point has none for an attribute, the frame's
    /// value counts.
    pub point_attrs: Option<Vec<Attributes>>,
    /// The input a frame came from, in the groups a match node makes.
    pub input: Option<String>,
}

impl Frame {
    /// A frame with the given attributes, columns and values per column.
    pub fn new(attrs: Attributes, columns: Vec<Column>, values: Vec<Vec<Option<f64>>>) -> Self {
        Self {
            attrs,
            columns,
            values,
            point_attrs: None,
            input: None,
        }
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.values.first().map_or(0, Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Index of the column `name`.
    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// The values of the column `name`.
    pub fn column_values(&self, name: &str) -> Option<&[Option<f64>]> {
        self.column(name).map(|i| self.values[i].as_slice())
    }

    /// The attribute `name` of point `i`: the point's own value, else the
    /// frame's.
    pub fn point_attr(&self, i: usize, name: &str) -> Option<&Attr> {
        self.point_attrs
            .as_ref()
            .and_then(|p| p.get(i))
            .and_then(|a| a.get(name))
            .or_else(|| self.attrs.get(name))
    }

    /// Checks that every column has one value per point and that the point
    /// attributes have one entry per point.
    pub fn check(&self) -> Result<(), String> {
        if self.values.len() != self.columns.len() {
            return Err(format!(
                "{} columns but {} lists of values",
                self.columns.len(),
                self.values.len()
            ));
        }
        let n = self.len();
        if let Some(c) = self.values.iter().position(|v| v.len() != n) {
            return Err(format!(
                "column {:?} has {} values, the frame {n} points",
                self.columns[c].name,
                self.values[c].len()
            ));
        }
        if let Some(p) = &self.point_attrs
            && p.len() != n
        {
            return Err(format!("{} point attributes for {n} points", p.len()));
        }
        Ok(())
    }
}

/// A group below the root: its key and its members.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// The attributes the group was formed by, with their values. `None`
    /// for frames without a value for the attribute.
    pub key: Vec<(String, Option<AttrValue>)>,
    pub tree: Tree,
}

/// An ordered list of frames or of groups, never both.
#[derive(Debug, Clone, PartialEq)]
pub enum Tree {
    Frames(Vec<Frame>),
    Groups(Vec<Group>),
}

impl Default for Tree {
    fn default() -> Self {
        Tree::Frames(Vec::new())
    }
}

impl Tree {
    /// Number of group levels above the frames: 0 for a flat list. An
    /// empty list of groups counts as one level.
    pub fn depth(&self) -> usize {
        match self {
            Tree::Frames(_) => 0,
            Tree::Groups(groups) => 1 + groups.first().map_or(0, |g| g.tree.depth()),
        }
    }

    /// All frames, in order.
    pub fn frames(&self) -> Vec<&Frame> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect<'a>(&'a self, out: &mut Vec<&'a Frame>) {
        match self {
            Tree::Frames(frames) => out.extend(frames),
            Tree::Groups(groups) => groups.iter().for_each(|g| g.tree.collect(out)),
        }
    }

    /// Checks that all frames are at the same depth and every frame is
    /// consistent ([`Frame::check`]).
    pub fn check(&self) -> Result<(), String> {
        let depth = self.depth();
        self.check_at(depth)
    }

    fn check_at(&self, depth: usize) -> Result<(), String> {
        match self {
            Tree::Frames(frames) if depth == 0 => frames.iter().try_for_each(Frame::check),
            Tree::Groups(groups) if depth > 0 => {
                groups.iter().try_for_each(|g| g.tree.check_at(depth - 1))
            }
            _ => Err("frames at different depths".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(run: &str, ys: &[Option<f64>]) -> Frame {
        let mut attrs = Attributes::new();
        attrs.insert("run.id".into(), Attr::new(AttrValue::Text(run.into())));
        Frame::new(attrs, vec![Column::new("y", "ms")], vec![ys.to_vec()])
    }

    #[test]
    fn a_point_falls_back_to_the_frame_attributes() {
        let mut f = frame("r1", &[Some(1.0), None]);
        assert_eq!(f.len(), 2);
        assert_eq!(f.column_values("y").unwrap(), &[Some(1.0), None]);
        let mut own = Attributes::new();
        own.insert("run.id".into(), Attr::new(AttrValue::Text("r2".into())));
        f.point_attrs = Some(vec![own, Attributes::new()]);
        assert_eq!(
            f.point_attr(0, "run.id").unwrap().value,
            AttrValue::Text("r2".into())
        );
        assert_eq!(
            f.point_attr(1, "run.id").unwrap().value,
            AttrValue::Text("r1".into())
        );
        assert!(f.point_attr(1, "sut.id").is_none());
        f.check().unwrap();
    }

    #[test]
    fn checks_find_inconsistent_frames_and_trees() {
        let mut f = frame("r1", &[Some(1.0)]);
        f.columns.push(Column::new("x", "s"));
        assert!(f.check().is_err());
        f.values.push(vec![Some(0.0), Some(1.0)]);
        assert!(f.check().unwrap_err().contains("column \"x\""));

        let flat = Tree::Frames(vec![frame("r1", &[Some(1.0)])]);
        assert_eq!(flat.depth(), 0);
        let nested = Tree::Groups(vec![
            Group {
                key: vec![("run.id".into(), Some(AttrValue::Text("r1".into())))],
                tree: flat.clone(),
            },
            Group {
                key: vec![("run.id".into(), None)],
                tree: Tree::Groups(vec![]),
            },
        ]);
        assert_eq!(nested.depth(), 1);
        assert!(nested.check().is_err());
        let nested = Tree::Groups(vec![Group {
            key: vec![],
            tree: flat,
        }]);
        nested.check().unwrap();
        assert_eq!(nested.frames().len(), 1);
    }

    #[test]
    fn json_values_keep_their_type() {
        assert_eq!(
            AttrValue::from_json(&serde_json::json!(10)),
            Some(AttrValue::Number(10.into()))
        );
        // Integers beyond f64 stay apart.
        let big = |s: &str| AttrValue::from_json(&serde_json::from_str(s).unwrap());
        assert_ne!(big("9007199254740992"), big("9007199254740993"));
        assert_eq!(
            big("9007199254740993").unwrap().as_f64(),
            Some(9007199254740992.0)
        );
        assert_eq!(AttrValue::number(2.5).unwrap().as_f64(), Some(2.5));
        assert_eq!(AttrValue::number(f64::NAN), None);
        assert_eq!(
            AttrValue::from_json(&serde_json::json!("10")),
            Some(AttrValue::Text("10".into()))
        );
        assert_eq!(
            AttrValue::from_json(&serde_json::json!([true, "a"])),
            Some(AttrValue::List(vec![
                AttrValue::Bool(true),
                AttrValue::Text("a".into())
            ]))
        );
        assert_eq!(AttrValue::from_json(&serde_json::json!(null)), None);
        assert_eq!(AttrValue::from_json(&serde_json::json!({"a": 1})), None);
    }
}
