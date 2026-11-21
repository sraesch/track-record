//! Side-by-side comparison along a dimension (FR-16), streamed as events
//! (NFR-7).
//!
//! The dimension is the run, the client device or one or more case
//! parameters. Everything else is the matching key of a row: the case
//! (scenario version, assets, remaining parameters after aliases) and,
//! unless devices are compared, the device. Parameters in
//! `exclude_from_key` are taken out of the key; their values are shown with
//! each run of a cell.
//!
//! A cell holds one aggregate per run in time order; samples of different
//! runs are never pooled. Differences to the reference use the latest run of
//! each cell. A difference against no value is not computable (FR-12a).
//! Cells whose latest run has another SUT version or shared environment
//! than the reference are marked.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::mpsc;
use track_record_core::dsl::Kind;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    CaseInfo, Cases, Dataset, Excluded, MetricInfo, Restrict, RunRef, StatsOptions, case_key,
    case_sort_key, default_confidence, eval_scalars, load_dataset, resolve_parameter, value_key,
};
use crate::error::{ApiError, ApiResult};
use crate::service::analysis::Stats;
use crate::service::derived::{self, Prepared};
use crate::service::query::Aliases;
use crate::service::runs::RunFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompareDimension {
    /// Run A vs. run B; values are run ids.
    Run,
    /// Client devices; values are device names after aliases.
    ClientDevice,
    /// Case parameters; values are objects such as `{"pipeline": "new"}`.
    Parameters,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareRequest {
    /// Runs to compare (FR-11): one run or a filtered set.
    #[serde(default)]
    pub filter: RunFilter,
    /// A scalar metric: uploaded, derived or any DSL expression.
    pub metric: String,
    pub unit: Option<String>,
    #[serde(default)]
    pub include_warmup: bool,
    pub dimension: CompareDimension,
    /// The compared parameters, for `dimension = parameters`.
    #[serde(default)]
    pub parameters: Vec<String>,
    /// Values of the dimension to show, in this order. Default: all values
    /// found (runs by start time, others sorted).
    #[schema(value_type = Option<Vec<Object>>)]
    pub values: Option<Vec<Value>>,
    /// The reference value; default the first value.
    #[schema(value_type = Option<Object>)]
    pub reference: Option<Value>,
    /// Parameters taken out of the matching key.
    #[serde(default)]
    pub exclude_from_key: Vec<String>,
    pub percentiles: Option<Vec<f64>>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CellRun {
    pub run: RunRef,
    pub stats: Stats,
    /// Values of the parameters taken out of the matching key, as objects.
    #[schema(value_type = Vec<Object>)]
    pub excluded_parameters: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Difference {
    /// Mean minus the reference mean.
    pub absolute: f64,
    /// `absolute` divided by the absolute reference mean; absent if the
    /// reference mean is 0.
    pub relative: Option<f64>,
    /// Mean divided by the reference mean (normalization, FR-13).
    pub ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareCell {
    /// The dimension value of this column.
    #[schema(value_type = Object)]
    pub value: Value,
    pub is_reference: bool,
    /// One aggregate per run, oldest first. The last is the one compared.
    pub runs: Vec<CellRun>,
    /// Latest run vs. the reference cell's latest run.
    pub difference: Option<Difference>,
    /// True if this cell or the reference has no value, so no difference
    /// can be computed (FR-12a).
    pub not_computable: bool,
    /// The latest run has another SUT version than the reference's.
    pub sut_version_differs: bool,
    /// The latest run has another shared environment than the reference's.
    pub shared_environment_differs: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRow {
    /// The matching key: the case without compared and excluded parameters.
    pub case: CaseInfo,
    /// Device name after aliases; absent when devices are compared.
    pub client_device: Option<String>,
    /// Only columns with results, in column order.
    pub cells: Vec<CompareCell>,
}

/// One line of the NDJSON stream.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompareEvent {
    /// First event: what is compared.
    Start {
        metric: MetricInfo,
        dimension: CompareDimension,
        #[schema(value_type = Vec<Object>)]
        values: Vec<Value>,
        #[schema(value_type = Option<Object>)]
        reference: Option<Value>,
        total_samples: usize,
    },
    /// Samples evaluated so far.
    Progress {
        done: usize,
        total: usize,
    },
    Row(CompareRow),
    /// Last event on success.
    Done {
        rows: usize,
        excluded: Excluded,
    },
    /// Last event on failure.
    Error {
        code: String,
        detail: String,
    },
}

/// Starts a comparison. Errors in the request are returned before
/// streaming; later errors end the stream with an `error` event.
pub async fn compare(pool: PgPool, req: CompareRequest) -> ApiResult<mpsc::Receiver<CompareEvent>> {
    let options = StatsOptions::new(&req.percentiles, req.confidence)?;
    if req.dimension == CompareDimension::Parameters && req.parameters.is_empty() {
        return Err(ApiError::bad_request(
            "dimension parameters needs the compared parameters in `parameters`",
        ));
    }
    let prepared = derived::prepare(&pool, &req.metric, req.unit.as_deref()).await?;
    if prepared.signature.kind != Kind::Scalar {
        return Err(ApiError::invalid(format!(
            "{} is a series; a comparison needs a scalar metric",
            req.metric
        )));
    }
    let aliases = Aliases::load(&pool).await?;
    let mut ds = load_dataset(
        &pool,
        &req.filter,
        req.include_warmup,
        &Restrict::default(),
        &aliases,
    )
    .await?;

    let column = |ds: &Dataset, i: usize| -> Value {
        let s = &ds.samples[i];
        match req.dimension {
            CompareDimension::Run => Value::String(s.run_id.to_string()),
            CompareDimension::ClientDevice => Value::String(s.client_device.clone()),
            CompareDimension::Parameters => {
                let tc = &ds.test_cases[&s.test_case_id];
                Value::Object(
                    req.parameters
                        .iter()
                        .map(|p| {
                            (
                                p.clone(),
                                tc.parameters.get(p).cloned().unwrap_or(Value::Null),
                            )
                        })
                        .collect(),
                )
            }
        }
    };
    let resolve = |v: &Value| -> Value {
        match (req.dimension, v) {
            (CompareDimension::ClientDevice, Value::String(name)) => {
                Value::String(aliases.resolve_device(name))
            }
            (CompareDimension::Parameters, Value::Object(m)) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), resolve_parameter(&aliases, k, v)))
                    .collect(),
            ),
            _ => v.clone(),
        }
    };

    let columns: Vec<Value> = (0..ds.samples.len()).map(|i| column(&ds, i)).collect();
    let values: Vec<Value> = match &req.values {
        Some(given) => {
            let mut out: Vec<Value> = Vec::new();
            for v in given.iter().map(resolve) {
                if !out.iter().any(|o| value_key(o) == value_key(&v)) {
                    out.push(v);
                }
            }
            out
        }
        None => {
            let mut found: BTreeMap<String, (Value, (chrono::DateTime<chrono::Utc>, String))> =
                BTreeMap::new();
            for (i, v) in columns.iter().enumerate() {
                let run = &ds.runs[&ds.samples[i].run_id];
                let order = match req.dimension {
                    CompareDimension::Run => (run.started_at, value_key(v)),
                    _ => (chrono::DateTime::<chrono::Utc>::MIN_UTC, value_key(v)),
                };
                found.entry(value_key(v)).or_insert((v.clone(), order));
            }
            let mut list: Vec<(Value, (chrono::DateTime<chrono::Utc>, String))> =
                found.into_values().collect();
            list.sort_by(|a, b| a.1.cmp(&b.1));
            list.into_iter().map(|(v, _)| v).collect()
        }
    };
    let reference = match &req.reference {
        Some(r) => {
            let r = resolve(r);
            if !values.iter().any(|v| value_key(v) == value_key(&r)) {
                return Err(ApiError::invalid(format!(
                    "the reference {r} is not one of the compared values"
                )));
            }
            Some(r)
        }
        None => values.first().cloned(),
    };

    // Keep only samples of the shown values.
    let index: HashMap<String, usize> = values
        .iter()
        .enumerate()
        .map(|(i, v)| (value_key(v), i))
        .collect();
    let mut column_of: Vec<usize> = Vec::new();
    let mut keep = Vec::new();
    for (i, v) in columns.iter().enumerate() {
        if let Some(c) = index.get(&value_key(v)) {
            column_of.push(*c);
            keep.push(i);
        }
    }
    ds.samples = keep.into_iter().map(|i| ds.samples[i].clone()).collect();

    let mut drop = req.exclude_from_key.clone();
    if req.dimension == CompareDimension::Parameters {
        drop.extend(req.parameters.iter().cloned());
    }
    let plan = Plan {
        dimension: req.dimension,
        excluded_keys: req.exclude_from_key.clone(),
        drop,
        values: values.clone(),
        reference: reference.as_ref().map(|r| index[&value_key(r)]),
        column_of,
        options,
    };
    let (tx, rx) = mpsc::channel(64);
    let start = CompareEvent::Start {
        metric: MetricInfo::of(&prepared),
        dimension: req.dimension,
        values,
        reference,
        total_samples: ds.samples.len(),
    };
    tokio::spawn(async move {
        if tx.send(start).await.is_err() {
            return;
        }
        if let Err(e) = run(&pool, &prepared, &ds, &plan, &tx).await {
            let detail = match &e {
                ApiError::Problem { detail, .. } => detail.clone(),
                ApiError::Internal(err) => {
                    tracing::error!(error = ?err, "comparison failed");
                    "internal server error".to_owned()
                }
            };
            let _ = tx
                .send(CompareEvent::Error {
                    code: e.code().to_owned(),
                    detail,
                })
                .await;
        }
    });
    Ok(rx)
}

struct Plan {
    dimension: CompareDimension,
    excluded_keys: Vec<String>,
    drop: Vec<String>,
    values: Vec<Value>,
    reference: Option<usize>,
    /// Column of each sample of the data set.
    column_of: Vec<usize>,
    options: StatsOptions,
}

#[derive(Default)]
struct CellData {
    /// run -> (values, excluded parameter values by canonical form)
    runs: HashMap<Uuid, (Vec<Option<f64>>, BTreeMap<String, Value>)>,
}

async fn run(
    pool: &PgPool,
    prepared: &Prepared,
    ds: &Dataset,
    plan: &Plan,
    tx: &mpsc::Sender<CompareEvent>,
) -> ApiResult<()> {
    let total = ds.samples.len();
    let progress_tx = tx.clone();
    let mut excluded = Excluded::default();
    let values = eval_scalars(pool, prepared, &ds.samples, &mut excluded, &mut |done| {
        // Progress is best effort: a slow reader skips some.
        let _ = progress_tx.try_send(CompareEvent::Progress { done, total });
    })
    .await?;

    let mut cases = Cases::default();
    // (case key, device) -> column -> cell
    let mut rows: HashMap<(String, Option<String>), HashMap<usize, CellData>> = HashMap::new();
    for (i, (s, v)) in ds.samples.iter().zip(&values).enumerate() {
        let Some(v) = v else { continue };
        let tc = &ds.test_cases[&s.test_case_id];
        let key = case_key(tc, &plan.drop);
        cases.add(&key, tc, &plan.drop);
        let device =
            (plan.dimension != CompareDimension::ClientDevice).then(|| s.client_device.clone());
        let cell = rows
            .entry((key, device))
            .or_default()
            .entry(plan.column_of[i])
            .or_default();
        let (vals, excluded_params) = cell.runs.entry(s.run_id).or_default();
        vals.push(*v);
        if !plan.excluded_keys.is_empty() {
            let obj = Value::Object(
                plan.excluded_keys
                    .iter()
                    .map(|k| {
                        (
                            k.clone(),
                            tc.parameters.get(k).cloned().unwrap_or(Value::Null),
                        )
                    })
                    .collect(),
            );
            excluded_params.insert(super::canonical(&obj), obj);
        }
    }

    let mut out: Vec<CompareRow> = rows
        .into_iter()
        .map(|((key, device), cells)| build_row(ds, plan, cases.get(&key), device, cells))
        .collect();
    out.sort_by(|a, b| {
        (case_sort_key(&a.case), &a.client_device).cmp(&(case_sort_key(&b.case), &b.client_device))
    });
    let count = out.len();
    for row in out {
        if tx.send(CompareEvent::Row(row)).await.is_err() {
            return Ok(());
        }
    }
    let _ = tx
        .send(CompareEvent::Done {
            rows: count,
            excluded,
        })
        .await;
    Ok(())
}

fn build_row(
    ds: &Dataset,
    plan: &Plan,
    case: CaseInfo,
    client_device: Option<String>,
    cells: HashMap<usize, CellData>,
) -> CompareRow {
    let mut columns: Vec<(usize, Vec<CellRun>)> = cells
        .into_iter()
        .map(|(col, data)| {
            let mut runs: Vec<CellRun> = data
                .runs
                .into_iter()
                .map(|(run, (vals, params))| CellRun {
                    run: ds.runs[&run].clone(),
                    stats: plan.options.summarize(&vals),
                    excluded_parameters: params.into_values().collect(),
                })
                .collect();
            runs.sort_by_key(|r| (r.run.started_at, r.run.id));
            (col, runs)
        })
        .collect();
    columns.sort_by_key(|(col, _)| *col);
    let reference = plan
        .reference
        .and_then(|r| columns.iter().find(|(c, _)| *c == r))
        .and_then(|(_, runs)| runs.last().cloned());
    let cells = columns
        .into_iter()
        .map(|(col, runs)| {
            let is_reference = plan.reference == Some(col);
            let latest = runs.last();
            let (mut difference, mut not_computable) = (None, false);
            let (mut sut_differs, mut env_differs) = (false, false);
            if let (Some(latest), Some(reference), false) = (latest, &reference, is_reference) {
                sut_differs = latest.run.sut_version.id != reference.run.sut_version.id;
                env_differs =
                    latest.run.shared_environment_id != reference.run.shared_environment_id;
                match (latest.stats.mean, reference.stats.mean) {
                    (Some(m), Some(r)) => {
                        difference = Some(Difference {
                            absolute: m - r,
                            relative: (r != 0.0).then(|| (m - r) / r.abs()),
                            ratio: (r != 0.0).then(|| m / r),
                        });
                    }
                    _ => not_computable = true,
                }
            }
            CompareCell {
                value: plan.values[col].clone(),
                is_reference,
                runs,
                difference,
                not_computable,
                sut_version_differs: sut_differs,
                shared_environment_differs: env_differs,
            }
        })
        .collect();
    CompareRow {
        case,
        client_device,
        cells,
    }
}
