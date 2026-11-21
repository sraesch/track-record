//! Export of analysis results (FR-20) as CSV, NDJSON or Parquet, streamed
//! (NFR-7).
//!
//! The view spec is the same as for aggregates. `level = samples` exports
//! one row per sample with its value (written chunk by chunk);
//! `level = aggregates` one row per run, case and device.

use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, Float64Builder, Int64Builder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use bytes::Bytes;
use parquet::arrow::ArrowWriter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::mpsc;
use track_record_core::dsl::Kind;
use utoipa::ToSchema;

use super::{
    AggregatesRequest, CHUNK, Dataset, Excluded, Restrict, RunRef, SampleRow, TestCaseRow,
    default_confidence, eval_scalars, load_dataset,
};
use crate::error::{ApiError, ApiResult};
use crate::service::derived::{self, Prepared};
use crate::service::query::Aliases;
use crate::service::runs::{AssetRef, RunFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportLevel {
    /// One row per sample.
    #[default]
    Samples,
    /// One row per run, case and client device.
    Aggregates,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    #[serde(default)]
    pub filter: RunFilter,
    /// A scalar metric: uploaded, derived or any DSL expression.
    pub metric: String,
    pub unit: Option<String>,
    #[serde(default)]
    pub include_warmup: bool,
    pub format: ExportFormat,
    #[serde(default)]
    pub level: ExportLevel,
    /// For `level = aggregates`.
    pub percentiles: Option<Vec<f64>>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

// Tables ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColType {
    Str,
    F64,
    I64,
    Bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Str(String),
    F64(f64),
    I64(i64),
    Bool(bool),
    Null,
}

impl Cell {
    fn opt_f64(v: Option<f64>) -> Cell {
        v.map_or(Cell::Null, Cell::F64)
    }

    fn opt_str(v: &Option<String>) -> Cell {
        v.clone().map_or(Cell::Null, Cell::Str)
    }
}

type Columns = Vec<(String, ColType)>;

/// Writes rows in one format, returning the bytes produced so far.
enum TableWriter {
    Csv { header_written: bool },
    Json,
    Parquet(Box<ArrowWriter<Vec<u8>>>),
}

impl TableWriter {
    fn new(format: ExportFormat, columns: &Columns) -> anyhow::Result<Self> {
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

    fn write(&mut self, columns: &Columns, rows: &[Vec<Cell>]) -> anyhow::Result<Bytes> {
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

    fn finish(self) -> anyhow::Result<Bytes> {
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
                    ColType::Str => DataType::Utf8,
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
                ColType::Str => {
                    let mut b = StringBuilder::new();
                    for c in cells {
                        match c {
                            Cell::Str(s) => b.append_value(s),
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

// Rows ------------------------------------------------------------------------------------

fn col(name: &str, ty: ColType) -> (String, ColType) {
    (name.to_owned(), ty)
}

fn run_columns() -> Columns {
    vec![
        col("run_id", ColType::Str),
        col("run_started_at", ColType::Str),
        col("run_is_valid", ColType::Bool),
        col("sut_commit", ColType::Str),
        col("sut_branch", ColType::Str),
        col("sut_build_config", ColType::Str),
        col("sut_tag", ColType::Str),
        col("shared_environment_id", ColType::I64),
    ]
}

fn run_cells(run: &RunRef) -> Vec<Cell> {
    vec![
        Cell::Str(run.id.to_string()),
        Cell::Str(run.started_at.to_rfc3339()),
        Cell::Bool(run.is_valid),
        Cell::Str(run.sut_version.commit.clone()),
        Cell::opt_str(&run.sut_version.branch),
        Cell::opt_str(&run.sut_version.build_config),
        Cell::opt_str(&run.sut_version.tag),
        Cell::I64(run.shared_environment_id),
    ]
}

/// Assets as `name@version` or `name#hash`, separated by `;`.
fn assets_text(assets: &[AssetRef]) -> String {
    assets
        .iter()
        .map(|a| match (&a.version, &a.content_hash) {
            (Some(v), _) => format!("{}@{v}", a.name),
            (None, Some(h)) => format!("{}#{h}", a.name),
            (None, None) => a.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn sample_columns() -> Columns {
    let mut c = run_columns();
    c.extend([
        col("case_execution_id", ColType::Str),
        col("test_case_id", ColType::I64),
        col("scenario", ColType::Str),
        col("scenario_version", ColType::I64),
        col("assets", ColType::Str),
        col("parameters", ColType::Str),
        col("client_device", ColType::Str),
        col("device_environment_id", ColType::I64),
        col("sample_id", ColType::Str),
        col("repetition", ColType::I64),
        col("is_warmup", ColType::Bool),
        col("metric", ColType::Str),
        col("unit", ColType::Str),
        col("value", ColType::F64),
        col("has_value", ColType::Bool),
    ]);
    c
}

fn sample_cells(
    ds: &Dataset,
    s: &SampleRow,
    tc: &TestCaseRow,
    p: &Prepared,
    value: Option<f64>,
) -> Vec<Cell> {
    let mut row = run_cells(&ds.runs[&s.run_id]);
    row.extend([
        Cell::Str(s.case_execution_id.to_string()),
        Cell::I64(tc.id),
        Cell::Str(tc.scenario.clone()),
        Cell::I64(i64::from(tc.scenario_version)),
        Cell::Str(assets_text(&tc.assets)),
        Cell::Str(Value::Object(tc.parameters.clone()).to_string()),
        Cell::Str(s.client_device.clone()),
        Cell::I64(s.device_environment_id),
        Cell::Str(s.sample_id.to_string()),
        Cell::I64(i64::from(s.repetition)),
        Cell::Bool(s.is_warmup),
        Cell::Str(p.expression.clone()),
        Cell::Str(p.signature.unit.clone()),
        Cell::opt_f64(value),
        Cell::Bool(value.is_some()),
    ]);
    row
}

fn percentile_name(p: f64) -> String {
    format!("p{p}")
}

/// A stream of export bytes. Errors after the start abort the response.
pub type ExportStream = mpsc::Receiver<Result<Bytes, std::io::Error>>;

/// Starts an export. Errors in the request are returned before streaming.
pub async fn export(pool: PgPool, req: ExportRequest) -> ApiResult<ExportStream> {
    let (tx, rx) = mpsc::channel(8);
    match req.level {
        ExportLevel::Aggregates => {
            let agg = super::aggregates(
                &pool,
                &AggregatesRequest {
                    filter: req.filter.clone(),
                    metric: req.metric.clone(),
                    unit: req.unit.clone(),
                    include_warmup: req.include_warmup,
                    percentiles: req.percentiles.clone(),
                    confidence: req.confidence,
                },
            )
            .await?;
            let percentiles: Vec<f64> = agg
                .groups
                .first()
                .map(|g| g.stats.percentiles.iter().map(|p| p.p).collect())
                .unwrap_or_else(|| {
                    req.percentiles
                        .clone()
                        .unwrap_or_else(|| super::DEFAULT_PERCENTILES.to_vec())
                });
            let mut columns = run_columns();
            columns.extend([
                col("scenario", ColType::Str),
                col("scenario_version", ColType::I64),
                col("assets", ColType::Str),
                col("parameters", ColType::Str),
                col("client_device", ColType::Str),
                col("metric", ColType::Str),
                col("unit", ColType::Str),
                col("n", ColType::I64),
                col("n_no_value", ColType::I64),
                col("mean", ColType::F64),
                col("median", ColType::F64),
                col("stddev", ColType::F64),
                col("min", ColType::F64),
                col("max", ColType::F64),
                col("ci_low", ColType::F64),
                col("ci_high", ColType::F64),
            ]);
            columns.extend(
                percentiles
                    .iter()
                    .map(|p| col(&percentile_name(*p), ColType::F64)),
            );
            let rows: Vec<Vec<Cell>> = agg
                .groups
                .iter()
                .map(|g| {
                    let mut row = run_cells(&g.run);
                    row.extend([
                        Cell::Str(g.case.scenario.clone()),
                        Cell::I64(i64::from(g.case.scenario_version)),
                        Cell::Str(assets_text(&g.case.assets)),
                        Cell::Str(g.case.parameters.to_string()),
                        Cell::Str(g.client_device.clone()),
                        Cell::Str(agg.metric.expression.clone()),
                        Cell::Str(agg.metric.unit.clone()),
                        Cell::I64(g.stats.n as i64),
                        Cell::I64(g.stats.n_no_value as i64),
                        Cell::opt_f64(g.stats.mean),
                        Cell::opt_f64(g.stats.median),
                        Cell::opt_f64(g.stats.stddev),
                        Cell::opt_f64(g.stats.min),
                        Cell::opt_f64(g.stats.max),
                        Cell::opt_f64(g.stats.ci_low),
                        Cell::opt_f64(g.stats.ci_high),
                    ]);
                    row.extend(g.stats.percentiles.iter().map(|p| Cell::opt_f64(p.value)));
                    row
                })
                .collect();
            let mut writer = TableWriter::new(req.format, &columns).map_err(ApiError::Internal)?;
            tokio::spawn(async move {
                let result = writer
                    .write(&columns, &rows)
                    .and_then(|first| Ok((first, writer.finish()?)));
                match result {
                    Ok((a, b)) => {
                        let _ = tx.send(Ok(a)).await;
                        let _ = tx.send(Ok(b)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                    }
                }
            });
        }
        ExportLevel::Samples => {
            if req.percentiles.is_some() {
                return Err(ApiError::bad_request(
                    "percentiles apply to level aggregates",
                ));
            }
            let prepared = derived::prepare(&pool, &req.metric, req.unit.as_deref()).await?;
            if prepared.signature.kind != Kind::Scalar {
                return Err(ApiError::invalid(format!(
                    "{} is a series; export needs a scalar metric",
                    req.metric
                )));
            }
            let aliases = Aliases::load(&pool).await?;
            let ds = load_dataset(
                &pool,
                &req.filter,
                req.include_warmup,
                &Restrict::default(),
                &aliases,
            )
            .await?;
            let columns = sample_columns();
            let writer = TableWriter::new(req.format, &columns).map_err(ApiError::Internal)?;
            tokio::spawn(async move {
                if let Err(e) = stream_samples(&pool, &prepared, &ds, columns, writer, &tx).await {
                    tracing::error!(error = %e, "export failed");
                    let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                }
            });
        }
    }
    Ok(rx)
}

async fn stream_samples(
    pool: &PgPool,
    prepared: &Prepared,
    ds: &Dataset,
    columns: Columns,
    mut writer: TableWriter,
    tx: &mpsc::Sender<Result<Bytes, std::io::Error>>,
) -> anyhow::Result<()> {
    let mut excluded = Excluded::default();
    if ds.samples.is_empty() {
        let head = writer.write(&columns, &[])?;
        let _ = tx.send(Ok(head)).await;
    }
    for chunk in ds.samples.chunks(CHUNK) {
        let values = eval_scalars(pool, prepared, chunk, &mut excluded, &mut |_| {})
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let rows: Vec<Vec<Cell>> = chunk
            .iter()
            .zip(values)
            .filter_map(|(s, v)| {
                v.map(|v| sample_cells(ds, s, &ds.test_cases[&s.test_case_id], prepared, v))
            })
            .collect();
        let bytes = writer.write(&columns, &rows)?;
        if tx.send(Ok(bytes)).await.is_err() {
            return Ok(());
        }
    }
    let tail = writer.finish()?;
    let _ = tx.send(Ok(tail)).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
