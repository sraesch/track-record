//! Tables written as CSV, NDJSON or Parquet (FR-20): the files of the
//! analysis export and of chart exports, written chunk by chunk (NFR-7).

use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, Float64Builder, Int64Builder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use bytes::Bytes;
use parquet::arrow::ArrowWriter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    /// Newline-delimited JSON, one object per row.
    Json,
    Parquet,
}

impl ExportFormat {
    pub fn content_type(self) -> &'static str {
        match self {
            ExportFormat::Csv => "text/csv; charset=utf-8",
            ExportFormat::Json => "application/x-ndjson",
            ExportFormat::Parquet => "application/vnd.apache.parquet",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "ndjson",
            ExportFormat::Parquet => "parquet",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColType {
    Str,
    F64,
    I64,
    Bool,
    /// Values of any JSON type: native in NDJSON, their JSON text in CSV
    /// and Parquet.
    Json,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Str(String),
    F64(f64),
    I64(i64),
    Bool(bool),
    /// A JSON value, kept as it is (see [`ColType::Json`]).
    Json(Value),
    Null,
}

impl Cell {
    pub fn opt_f64(v: Option<f64>) -> Cell {
        v.map_or(Cell::Null, Cell::F64)
    }

    pub fn opt_str(v: &Option<String>) -> Cell {
        v.clone().map_or(Cell::Null, Cell::Str)
    }
}

pub type Columns = Vec<(String, ColType)>;

/// Writes rows in one format, returning the bytes produced so far.
pub enum TableWriter {
    Csv { header_written: bool },
    Json,
    Parquet(Box<ArrowWriter<Vec<u8>>>),
}

impl TableWriter {
    pub fn new(format: ExportFormat, columns: &Columns) -> anyhow::Result<Self> {
        Ok(match format {
            ExportFormat::Csv => TableWriter::Csv {
                header_written: false,
            },
            ExportFormat::Json => TableWriter::Json,
            ExportFormat::Parquet => TableWriter::Parquet(Box::new(ArrowWriter::try_new(
                Vec::new(),
                schema(columns),
                None,
            )?)),
        })
    }

    pub fn write(&mut self, columns: &Columns, rows: &[Vec<Cell>]) -> anyhow::Result<Bytes> {
        match self {
            TableWriter::Csv { header_written } => {
                let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
                if !*header_written {
                    w.write_record(columns.iter().map(|c| c.0.as_str()))?;
                    *header_written = true;
                }
                for row in rows {
                    w.write_record(row.iter().map(|c| match c {
                        Cell::Str(s) => s.clone(),
                        Cell::Json(v) => v.to_string(),
                        Cell::F64(v) => v.to_string(),
                        Cell::I64(v) => v.to_string(),
                        Cell::Bool(v) => v.to_string(),
                        Cell::Null => String::new(),
                    }))?;
                }
                Ok(Bytes::from(w.into_inner()?))
            }
            TableWriter::Json => {
                let mut out = Vec::new();
                for row in rows {
                    let obj: serde_json::Map<String, Value> = columns
                        .iter()
                        .zip(row)
                        .map(|((name, _), c)| {
                            (
                                name.clone(),
                                match c {
                                    Cell::Str(s) => Value::String(s.clone()),
                                    Cell::Json(v) => v.clone(),
                                    Cell::F64(v) => serde_json::Number::from_f64(*v)
                                        .map_or(Value::Null, Value::Number),
                                    Cell::I64(v) => Value::from(*v),
                                    Cell::Bool(v) => Value::Bool(*v),
                                    Cell::Null => Value::Null,
                                },
                            )
                        })
                        .collect();
                    serde_json::to_writer(&mut out, &obj)?;
                    out.push(b'\n');
                }
                Ok(Bytes::from(out))
            }
            TableWriter::Parquet(w) => {
                if !rows.is_empty() {
                    w.write(&batch(columns, rows)?)?;
                    // Each chunk becomes a row group; its bytes go out now.
                    w.flush()?;
                }
                Ok(Bytes::from(std::mem::take(w.inner_mut())))
            }
        }
    }

    pub fn finish(self) -> anyhow::Result<Bytes> {
        match self {
            TableWriter::Parquet(w) => Ok(Bytes::from(w.into_inner()?)),
            _ => Ok(Bytes::new()),
        }
    }
}

fn schema(columns: &Columns) -> Arc<Schema> {
    Arc::new(Schema::new(
        columns
            .iter()
            .map(|(name, ty)| {
                let dt = match ty {
                    ColType::Str | ColType::Json => DataType::Utf8,
                    ColType::F64 => DataType::Float64,
                    ColType::I64 => DataType::Int64,
                    ColType::Bool => DataType::Boolean,
                };
                Field::new(name, dt, true)
            })
            .collect::<Vec<_>>(),
    ))
}

fn batch(columns: &Columns, rows: &[Vec<Cell>]) -> anyhow::Result<RecordBatch> {
    let arrays: Vec<ArrayRef> = columns
        .iter()
        .enumerate()
        .map(|(i, (_, ty))| -> ArrayRef {
            let cells = rows.iter().map(|r| &r[i]);
            match ty {
                ColType::Str | ColType::Json => {
                    let mut b = StringBuilder::new();
                    for c in cells {
                        match c {
                            Cell::Str(s) => b.append_value(s),
                            Cell::Json(v) => b.append_value(v.to_string()),
                            _ => b.append_null(),
                        }
                    }
                    Arc::new(b.finish())
                }
                ColType::F64 => {
                    let mut b = Float64Builder::new();
                    for c in cells {
                        match c {
                            Cell::F64(v) => b.append_value(*v),
                            _ => b.append_null(),
                        }
                    }
                    Arc::new(b.finish())
                }
                ColType::I64 => {
                    let mut b = Int64Builder::new();
                    for c in cells {
                        match c {
                            Cell::I64(v) => b.append_value(*v),
                            _ => b.append_null(),
                        }
                    }
                    Arc::new(b.finish())
                }
                ColType::Bool => {
                    let mut b = BooleanBuilder::new();
                    for c in cells {
                        match c {
                            Cell::Bool(v) => b.append_value(*v),
                            _ => b.append_null(),
                        }
                    }
                    Arc::new(b.finish())
                }
            }
        })
        .collect();
    Ok(RecordBatch::try_new(schema(columns), arrays)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: ColType) -> (String, ColType) {
        (name.to_owned(), ty)
    }

    fn columns() -> Columns {
        vec![
            col("name", ColType::Str),
            col("value", ColType::F64),
            col("n", ColType::I64),
            col("ok", ColType::Bool),
        ]
    }

    fn rows() -> Vec<Vec<Cell>> {
        vec![
            vec![
                Cell::Str("a,\"b\"".into()),
                Cell::F64(1.5),
                Cell::I64(2),
                Cell::Bool(true),
            ],
            vec![
                Cell::Str("c".into()),
                Cell::Null,
                Cell::I64(3),
                Cell::Bool(false),
            ],
        ]
    }

    #[test]
    fn csv_quotes_and_leaves_nulls_empty() {
        let mut w = TableWriter::new(ExportFormat::Csv, &columns()).unwrap();
        let mut out = w.write(&columns(), &rows()[..1]).unwrap().to_vec();
        out.extend(w.write(&columns(), &rows()[1..]).unwrap());
        out.extend(w.finish().unwrap());
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "name,value,n,ok\n\"a,\"\"b\"\"\",1.5,2,true\nc,,3,false\n"
        );
    }

    #[test]
    fn json_lines() {
        let mut w = TableWriter::new(ExportFormat::Json, &columns()).unwrap();
        let out = w.write(&columns(), &rows()).unwrap();
        let lines: Vec<Value> = String::from_utf8(out.to_vec())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines[1]["value"], Value::Null);
        assert_eq!(lines[0]["n"], 2);
    }

    #[test]
    fn parquet_round_trip_in_row_groups() {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        let mut w = TableWriter::new(ExportFormat::Parquet, &columns()).unwrap();
        let mut out = w.write(&columns(), &rows()[..1]).unwrap().to_vec();
        out.extend(w.write(&columns(), &rows()[1..]).unwrap());
        out.extend(w.finish().unwrap());
        let reader = ParquetRecordBatchReaderBuilder::try_new(Bytes::from(out))
            .unwrap()
            .build()
            .unwrap();
        let batches: Vec<RecordBatch> = reader.map(|b| b.unwrap()).collect();
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total, 2);
        assert_eq!(batches[0].schema().field(1).name(), "value");
    }
}
