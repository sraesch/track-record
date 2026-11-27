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

use std::cmp::Ordering;
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::Value;

/// A column of a frame: its name and unit (`""` for none). The `x` of a
/// series also keeps its dimension (`time`, `bytes`, ...): two x columns in
/// the same unit but of different dimensions are different columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub unit: String,
    /// The x-dimension of a series' `x`; `""` for other columns.
    pub dimension: String,
}

impl Column {
    pub fn new(name: impl Into<String>, unit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            unit: unit.into(),
            dimension: String::new(),
        }
    }

    /// The `x` column of a series with its unit and dimension.
    pub fn x(unit: impl Into<String>, dimension: impl Into<String>) -> Self {
        Self {
            dimension: dimension.into(),
            ..Self::new("x", unit)
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

    /// The order in which nodes sort and group values: numbers, times,
    /// texts, booleans, lists; within a type by value. Numbers compare by
    /// value, as the analysis compares parameters: `10` equals `10.0`, and
    /// integers beyond the precision of `f64` stay apart.
    pub fn total_cmp(&self, other: &AttrValue) -> Ordering {
        use AttrValue as V;
        match (self, other) {
            (V::Number(a), V::Number(b)) => cmp_numbers(a, b),
            (V::Time(a), V::Time(b)) => a.cmp(b),
            (V::Text(a), V::Text(b)) => a.cmp(b),
            (V::Bool(a), V::Bool(b)) => a.cmp(b),
            (V::List(a), V::List(b)) => {
                for (x, y) in a.iter().zip(b) {
                    let o = x.total_cmp(y);
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                a.len().cmp(&b.len())
            }
            _ => self.rank().cmp(&other.rank()),
        }
    }

    /// True if the two values are the same by [`AttrValue::total_cmp`].
    pub fn same(&self, other: &AttrValue) -> bool {
        self.total_cmp(other) == Ordering::Equal
    }

    fn rank(&self) -> u8 {
        match self {
            AttrValue::Number(_) => 0,
            AttrValue::Time(_) => 1,
            AttrValue::Text(_) => 2,
            AttrValue::Bool(_) => 3,
            AttrValue::List(_) => 4,
        }
    }
}

/// Compares two JSON numbers exactly: integers as integers, an integer and
/// a float without rounding the integer, and `-0.0` equal to `0`.
fn cmp_numbers(a: &serde_json::Number, b: &serde_json::Number) -> Ordering {
    match (integer(a), integer(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(x), None) => cmp_int_float(x, b.as_f64().unwrap_or(0.0)),
        (None, Some(y)) => cmp_int_float(y, a.as_f64().unwrap_or(0.0)).reverse(),
        (None, None) => {
            let (x, y) = (a.as_f64().unwrap_or(0.0), b.as_f64().unwrap_or(0.0));
            // JSON numbers are finite, so only the sign of zero can differ.
            x.partial_cmp(&y).unwrap_or(Ordering::Equal)
        }
    }
}

fn integer(n: &serde_json::Number) -> Option<i128> {
    n.as_i64()
        .map(i128::from)
        .or_else(|| n.as_u64().map(i128::from))
}

/// The order of an integer and a finite float, exactly.
fn cmp_int_float(i: i128, f: f64) -> Ordering {
    // Floats at or beyond ±2^127 lie beyond every i64 or u64.
    const LIMIT: f64 = 170_141_183_460_469_231_731_687_303_715_884_105_728.0;
    if f >= LIMIT {
        return Ordering::Less;
    }
    if f <= -LIMIT {
        return Ordering::Greater;
    }
    let whole = f.trunc();
    // A float of at most 2^127 in magnitude with no fraction is an exact
    // i128.
    match i.cmp(&(whole as i128)) {
        Ordering::Equal => 0.0f64.partial_cmp(&(f - whole)).unwrap_or(Ordering::Equal),
        o => o,
    }
}

/// Orders optional values with the missing ones last, also when `desc`
/// reverses the order of the others.
pub fn cmp_missing_last(a: Option<&AttrValue>, b: Option<&AttrValue>, desc: bool) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            let o = x.total_cmp(y);
            if desc { o.reverse() } else { o }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
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

impl Attr {
    /// Same value and unit.
    pub fn same(&self, other: &Attr) -> bool {
        self.unit == other.unit && self.value.same(&other.value)
    }
}

/// Attributes by name.
pub type Attributes = BTreeMap<String, Attr>;

/// The attributes that all frames share: same value and unit. A frame
/// merged from them (reduce, concatenate, combine) keeps these.
pub fn common_attributes<'a>(frames: impl IntoIterator<Item = &'a Frame>) -> Attributes {
    let mut frames = frames.into_iter();
    let Some(first) = frames.next() else {
        return Attributes::new();
    };
    let mut common = first.attrs.clone();
    for f in frames {
        common.retain(|name, attr| f.attrs.get(name).is_some_and(|a| a.same(attr)));
        if common.is_empty() {
            break;
        }
    }
    common
}

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

    /// Keeps the points at `indices`, in that order: the values of every
    /// column and the point attributes move together.
    pub fn select_points(&mut self, indices: &[usize]) {
        for column in &mut self.values {
            *column = indices.iter().map(|&i| column[i]).collect();
        }
        if let Some(points) = &mut self.point_attrs {
            let mut old: Vec<Option<Attributes>> =
                std::mem::take(points).into_iter().map(Some).collect();
            // Indices do not repeat: a point is kept or moved, never copied.
            *points = indices
                .iter()
                .map(|&i| old[i].take().unwrap_or_default())
                .collect();
        }
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
    /// The attributes the group was formed by, with their values and units.
    /// An attribute the key names is `None` for frames without a value; of
    /// the attributes a family stands for, only those the frames have.
    pub key: Vec<(String, Option<Attr>)>,
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
                key: vec![(
                    "run.id".into(),
                    Some(Attr::new(AttrValue::Text("r1".into()))),
                )],
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
    fn values_order_by_type_then_value_and_numbers_by_value() {
        let j = |v: serde_json::Value| AttrValue::from_json(&v).unwrap();
        assert!(j(serde_json::json!(10)).same(&j(serde_json::json!(10.0))));
        assert!(!j(serde_json::json!(10)).same(&j(serde_json::json!("10"))));
        let big = |s: &str| AttrValue::from_json(&serde_json::from_str(s).unwrap()).unwrap();
        assert_eq!(
            big("9007199254740992").total_cmp(&big("9007199254740993")),
            Ordering::Less
        );
        // Exact across integers and floats, and -0.0 is 0.
        let around = [
            big("9007199254740992.0"),
            big("9007199254740993"),
            big("9007199254740994.0"),
        ];
        assert_eq!(around[0].total_cmp(&around[1]), Ordering::Less);
        assert_eq!(around[1].total_cmp(&around[2]), Ordering::Less);
        assert_eq!(around[1].total_cmp(&around[0]), Ordering::Greater);
        assert!(!around[0].same(&around[1]));
        assert!(j(serde_json::json!(-0.0)).same(&j(serde_json::json!(0))));
        assert!(j(serde_json::json!(-0.0)).same(&j(serde_json::json!(0.0))));
        assert_eq!(
            j(serde_json::json!(2.5)).total_cmp(&j(serde_json::json!(2))),
            Ordering::Greater
        );
        assert_eq!(
            j(serde_json::json!(-2.5)).total_cmp(&j(serde_json::json!(-2))),
            Ordering::Less
        );
        assert_eq!(
            big("18446744073709551615").total_cmp(&j(serde_json::json!(1e300))),
            Ordering::Less
        );
        let mut values = [
            j(serde_json::json!([true])),
            AttrValue::Bool(false),
            j(serde_json::json!("a")),
            AttrValue::Time(DateTime::<Utc>::UNIX_EPOCH),
            j(serde_json::json!(2.5)),
            j(serde_json::json!(-3)),
        ];
        values.sort_by(AttrValue::total_cmp);
        assert_eq!(values[0], j(serde_json::json!(-3)));
        assert_eq!(values[2], AttrValue::Time(DateTime::<Utc>::UNIX_EPOCH));
        assert_eq!(values[5], j(serde_json::json!([true])));
        let x = j(serde_json::json!(1));
        assert_eq!(cmp_missing_last(Some(&x), None, true), Ordering::Less);
        assert_eq!(cmp_missing_last(None, Some(&x), false), Ordering::Greater);
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
