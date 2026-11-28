//! Export of the data that reaches the visualization of a chart (FR-20,
//! ADR 0022, "Evaluation"): one row per point with its attributes and
//! columns, as CSV, NDJSON or Parquet.
//!
//! The columns of the file are, in this order: the attributes of the frames
//! and points (by name), `input` if a frame came from a match, then the
//! columns of the frames in the order they first appear. An attribute or
//! column with a unit gets a `unit(<name>)` column after it; no attribute
//! or chart column can have such a name.
//!
//! A column's type is the type all its values share: booleans, integers
//! (that fit 64 bits), numbers (exact as 64-bit floats), texts, or times
//! (as RFC 3339 text). Values never lose their type or digits: a column of
//! other values, or of mixed types (`10` and `"10"`), holds JSON values,
//! native in NDJSON and as JSON text in CSV and Parquet (`10`, `"10"`).

use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;
use tokio::sync::mpsc;
use track_record_core::chart::{Attr, AttrValue, Frame, Tree};

use super::model::attr_value;
use crate::service::table::{Cell, ColType, Columns, ExportFormat, TableWriter};

/// Rows written at a time.
const CHUNK_ROWS: usize = 10_000;

/// Integers up to this size are exact as `f64`.
const EXACT_F64: u64 = 1 << 53;

/// The kinds of values an attribute has over all points.
#[derive(Debug, Default, Clone, Copy)]
struct Seen {
    bool: bool,
    /// Integers that fit `i64`.
    int: bool,
    /// Numbers that are not integers.
    float: bool,
    /// Integers that do not fit `i64`.
    huge: bool,
    /// Integers that are not exact as `f64`.
    wide: bool,
    text: bool,
    /// An empty text, which CSV cannot tell from no value.
    empty: bool,
    time: bool,
    other: bool,
    unit: bool,
}

impl Seen {
    fn add(&mut self, a: &Attr) {
        match &a.value {
            AttrValue::Bool(_) => self.bool = true,
            AttrValue::Number(n) => match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => {
                    self.int = true;
                    self.wide |= i.unsigned_abs() > EXACT_F64;
                }
                (None, Some(_)) => self.huge = true,
                (None, None) => self.float = true,
            },
            AttrValue::Text(t) => {
                self.text = true;
                self.empty |= t.is_empty();
            }
            AttrValue::Time(_) => self.time = true,
            AttrValue::List(_) => self.other = true,
        }
        self.unit |= !a.unit.is_empty();
    }

    /// The column type; for `csv`, a text column with an empty text holds
    /// JSON text, so that `""` stays apart from an empty cell.
    fn col_type(self, csv: bool) -> ColType {
        let Seen {
            bool,
            int,
            float,
            huge,
            wide,
            text,
            empty,
            time,
            other,
            ..
        } = self;
        let numbers = int || float || huge;
        let kinds = [bool, numbers, text, time, other]
            .into_iter()
            .filter(|k| *k)
            .count();
        if kinds > 1 || other {
            return ColType::Json;
        }
        if bool {
            ColType::Bool
        } else if int && !float && !huge {
            ColType::I64
        } else if float && !huge && !wide {
            ColType::F64
        } else if numbers || (csv && text && empty) {
            // Integers beyond i64, or beyond the precision of f64 next to
            // fractions: exact only as JSON.
            ColType::Json
        } else {
            ColType::Str
        }
    }
}

/// The name of the unit column of an attribute or chart column.
fn unit_column(name: &str) -> String {
    format!("unit({name})")
}

/// The layout of the file: which attributes and columns it has.
struct Layout {
    attrs: Vec<(String, Seen)>,
    input: bool,
    /// Columns by name, with whether any has a unit.
    columns: Vec<(String, bool)>,
    csv: bool,
}

impl Layout {
    fn of(frames: &[&Frame], format: ExportFormat) -> Self {
        let mut attrs: BTreeMap<String, Seen> = BTreeMap::new();
        let mut columns: Vec<(String, bool)> = Vec::new();
        let mut input = false;
        for f in frames {
            input |= f.input.is_some();
            for (name, a) in &f.attrs {
                attrs.entry(name.clone()).or_default().add(a);
            }
            for p in f.point_attrs.iter().flatten() {
                for (name, a) in p {
                    attrs.entry(name.clone()).or_default().add(a);
                }
            }
            for c in &f.columns {
                match columns.iter_mut().find(|(n, _)| *n == c.name) {
                    Some((_, unit)) => *unit |= !c.unit.is_empty(),
                    None => columns.push((c.name.clone(), !c.unit.is_empty())),
                }
            }
        }
        Self {
            attrs: attrs.into_iter().collect(),
            input,
            csv: format == ExportFormat::Csv,
            columns,
        }
    }

    fn columns(&self) -> Columns {
        let mut out = Columns::new();
        for (name, seen) in &self.attrs {
            out.push((name.clone(), seen.col_type(self.csv)));
            if seen.unit {
                out.push((unit_column(name), ColType::Str));
            }
        }
        if self.input {
            out.push(("input".into(), ColType::Str));
        }
        for (name, unit) in &self.columns {
            out.push((name.clone(), ColType::F64));
            if *unit {
                out.push((unit_column(name), ColType::Str));
            }
        }
        out
    }

    fn row(&self, f: &Frame, i: usize) -> Vec<Cell> {
        let mut row = Vec::new();
        for (name, seen) in &self.attrs {
            let a = f.point_attr(i, name);
            row.push(match a {
                None => Cell::Null,
                Some(a) => cell(&a.value, seen.col_type(self.csv)),
            });
            if seen.unit {
                row.push(a.map_or(Cell::Null, |a| Cell::Str(a.unit.clone())));
            }
        }
        if self.input {
            row.push(Cell::opt_str(&f.input));
        }
        for (name, unit) in &self.columns {
            let c = f.column(name);
            row.push(Cell::opt_f64(c.and_then(|c| f.values[c][i])));
            if *unit {
                row.push(c.map_or(Cell::Null, |c| Cell::Str(f.columns[c].unit.clone())));
            }
        }
        row
    }
}

fn cell(v: &AttrValue, ty: ColType) -> Cell {
    match (v, ty) {
        (AttrValue::Bool(b), ColType::Bool) => Cell::Bool(*b),
        (AttrValue::Number(n), ColType::I64) => n.as_i64().map_or(Cell::Null, Cell::I64),
        (AttrValue::Number(n), ColType::F64) => n.as_f64().map_or(Cell::Null, Cell::F64),
        (AttrValue::Text(t), ColType::Str) => Cell::Str(t.clone()),
        (AttrValue::Time(t), ColType::Str) => Cell::Str(t.to_rfc3339()),
        (v, _) => Cell::Json(attr_value(v)),
    }
}

/// Writes the frames of `tree` as a file in `format`, sending the bytes as
/// they are made. Stops quietly when the receiver is gone.
pub async fn write(
    tree: &Tree,
    format: ExportFormat,
    tx: &mpsc::Sender<Result<Bytes, std::io::Error>>,
) -> anyhow::Result<()> {
    let frames = tree.frames();
    let layout = Layout::of(&frames, format);
    let columns = layout.columns();
    check_unique(&columns)?;
    let mut writer = TableWriter::new(format, &columns)?;
    let mut rows = Vec::with_capacity(CHUNK_ROWS);
    let mut wrote = false;
    for f in &frames {
        for i in 0..f.len() {
            rows.push(layout.row(f, i));
            if rows.len() == CHUNK_ROWS {
                if tx.send(Ok(writer.write(&columns, &rows)?)).await.is_err() {
                    return Ok(());
                }
                rows.clear();
                wrote = true;
            }
        }
    }
    if !rows.is_empty() || !wrote {
        // The header of a CSV file without rows, too.
        if tx.send(Ok(writer.write(&columns, &rows)?)).await.is_err() {
            return Ok(());
        }
    }
    let _ = tx.send(Ok(writer.finish()?)).await;
    Ok(())
}

/// A column of the chart named like an attribute without a family
/// (`source`, `metric`) would give the file two columns of one name.
fn check_unique(columns: &Columns) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for (name, _) in columns {
        if !seen.insert(name.as_str()) {
            anyhow::bail!(
                "the file would have two columns {name:?}: a column of the chart has the name \
                 of an attribute; rename it in the node that makes it"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use track_record_core::chart::Attr;

    use super::*;

    fn col_type(values: &[serde_json::Value]) -> ColType {
        let mut seen = Seen::default();
        for v in values {
            seen.add(&Attr::new(AttrValue::from_json(v).unwrap()));
        }
        seen.col_type(false)
    }

    #[test]
    fn columns_keep_the_type_and_digits_of_their_values() {
        assert_eq!(col_type(&[json!(1), json!(i64::MAX)]), ColType::I64);
        assert_eq!(col_type(&[json!(1), json!(2.5)]), ColType::F64);
        assert_eq!(col_type(&[json!(true), json!(false)]), ColType::Bool);
        assert_eq!(col_type(&[json!("a"), json!("b")]), ColType::Str);
        // Not exact as i64 or f64: JSON.
        assert_eq!(col_type(&[json!(u64::MAX)]), ColType::Json);
        assert_eq!(
            col_type(&[json!(9_007_199_254_740_993_i64), json!(0.5)]),
            ColType::Json
        );
        // Mixed types stay apart.
        assert_eq!(col_type(&[json!(10), json!("10")]), ColType::Json);
        assert_eq!(col_type(&[json!(["a"])]), ColType::Json);
    }

    #[test]
    fn csv_keeps_an_empty_text_apart_from_no_value() {
        let mut seen = Seen::default();
        seen.add(&Attr::new(AttrValue::Text(String::new())));
        seen.add(&Attr::new(AttrValue::Text("a".into())));
        assert_eq!(seen.col_type(true), ColType::Json);
        assert_eq!(seen.col_type(false), ColType::Str);
        assert_eq!(
            cell(&AttrValue::Text(String::new()), ColType::Json),
            Cell::Json(json!(""))
        );
    }

    #[test]
    fn json_cells_keep_the_value() {
        let big = AttrValue::from_json(&json!(u64::MAX)).unwrap();
        assert_eq!(cell(&big, ColType::Json), Cell::Json(json!(u64::MAX)));
        let text = AttrValue::Text("10".into());
        assert_eq!(cell(&text, ColType::Json), Cell::Json(json!("10")));
    }
}
