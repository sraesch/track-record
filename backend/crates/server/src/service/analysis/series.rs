//! Series views (FR-19): overlay series of many samples, resample them onto
//! a common x grid, aggregate them across samples (median with a
//! percentile band) and downsample for display.
//!
//! The metric is a series expression: an uploaded series (`series.fps`), a
//! derived series metric or any series DSL expression. y values are in the
//! output unit, x values are converted to one x unit for all samples.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use track_record_core::dsl::Kind;
use track_record_core::dsl::eval::Value as DslValue;
use track_record_core::series::{Interp, Series, aggregate_across, extent, grid};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    CHUNK, CaseInfo, Cases, Dataset, Excluded, MetricInfo, Restrict, RunRef, case_key,
    case_sort_key, load_dataset,
};
use crate::error::{ApiError, ApiResult};
use crate::service::derived::{self, Prepared};
use crate::service::query::Aliases;
use crate::service::runs::RunFilter;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterpParam {
    #[default]
    Linear,
    Step,
}

impl From<InterpParam> for Interp {
    fn from(i: InterpParam) -> Self {
        match i {
            InterpParam::Linear => Interp::Linear,
            InterpParam::Step => Interp::Step,
        }
    }
}

/// A common x grid: `points` evenly spaced values from `from` to `to`.
/// Without `from`/`to` the grid covers the series (per group when
/// aggregating).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GridSpec {
    pub from: Option<f64>,
    pub to: Option<f64>,
    /// Number of grid points, 2 to 10000 (default 200).
    pub points: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SeriesViewRequest {
    #[serde(default)]
    pub filter: RunFilter,
    /// A series: `series.<name>`, a derived series metric or a series
    /// expression.
    pub metric: String,
    /// Unit of y.
    pub unit: Option<String>,
    /// Unit of x; default the x unit of the most recent series.
    pub x_unit: Option<String>,
    #[serde(default)]
    pub include_warmup: bool,
    /// Resample onto this grid. Required for alignment in an overlay;
    /// aggregation always uses a grid.
    pub grid: Option<GridSpec>,
    /// Interpolation for resampling.
    #[serde(default)]
    pub interp: InterpParam,
    /// Overlay without grid: downsample each series to at most this many
    /// points (default 2000).
    pub max_points: Option<usize>,
    /// Most series in an overlay (default 500, at most 5000).
    pub max_series: Option<usize>,
    /// Aggregation: percentiles per grid point (default 25, 50, 75).
    pub percentiles: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverlaySeries {
    pub run: RunRef,
    pub case: CaseInfo,
    pub client_device: String,
    pub case_execution_id: Uuid,
    pub sample_id: Uuid,
    pub repetition: i32,
    pub is_warmup: bool,
    /// x values; absent when resampled onto the grid.
    pub x: Option<Vec<f64>>,
    /// y values; null where the series has no value at a grid point.
    pub y: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverlayResponse {
    pub metric: MetricInfo,
    pub x_unit: String,
    pub x_dimension: String,
    /// The common grid, if one was requested.
    pub grid: Option<Vec<f64>>,
    pub series: Vec<OverlaySeries>,
    /// True if more series matched than `max_series`.
    pub truncated: bool,
    pub excluded: Excluded,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BandValues {
    pub p: f64,
    pub values: Vec<Option<f64>>,
}

/// Series of one run, case and device aggregated across samples.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeriesGroup {
    pub run: RunRef,
    pub case: CaseInfo,
    pub client_device: String,
    pub n_series: usize,
    pub x: Vec<f64>,
    /// Number of series with a value at each grid point.
    pub n: Vec<usize>,
    pub percentiles: Vec<BandValues>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeriesAggregateResponse {
    pub metric: MetricInfo,
    pub x_unit: String,
    pub x_dimension: String,
    pub groups: Vec<SeriesGroup>,
    pub excluded: Excluded,
}

struct Loaded {
    prepared: Prepared,
    ds: Dataset,
    x_unit: String,
    /// (sample index, series with x in `x_unit`)
    series: Vec<(usize, Series)>,
    excluded: Excluded,
    truncated: bool,
}

async fn load(pool: &PgPool, req: &SeriesViewRequest, max_series: usize) -> ApiResult<Loaded> {
    let mut conn = pool.acquire().await?;
    let prepared = derived::prepare(&mut conn, &req.metric, req.unit.as_deref()).await?;
    if prepared.signature.kind != Kind::Series {
        return Err(ApiError::invalid(format!(
            "{} is a scalar; this view needs a series, e.g. series.<name>",
            req.metric
        )));
    }
    let x_unit = req
        .x_unit
        .clone()
        .unwrap_or_else(|| prepared.signature.x_unit.clone());
    if prepared
        .catalogue
        .conversion_factor(&prepared.signature.x_unit, &x_unit)
        .is_none()
    {
        return Err(ApiError::invalid(format!(
            "x is in {:?}, which cannot be converted to {x_unit:?}",
            prepared.signature.x_unit
        )));
    }
    let aliases = Aliases::load(&mut *conn).await?;
    let ds = load_dataset(
        &mut conn,
        &req.filter,
        req.include_warmup,
        &Restrict::default(),
        &aliases,
    )
    .await?;

    let mut excluded = Excluded::default();
    let mut series = Vec::new();
    let mut truncated = false;
    {
        let mut evaluator = prepared.evaluator();
        'chunks: for (c, chunk) in ds.samples.chunks(CHUNK).enumerate() {
            let ids: Vec<Uuid> = chunk.iter().map(|s| s.sample_id).collect();
            let data = derived::load_sample_data(&mut conn, &ids, evaluator.inputs()).await?;
            for (j, s) in chunk.iter().enumerate() {
                match evaluator.evaluate(&data[&s.sample_id]) {
                    Ok(out) => {
                        let DslValue::Series(Some(points)) = out.value else {
                            continue;
                        };
                        let Some(f) = prepared
                            .catalogue
                            .conversion_factor(&out.signature.x_unit, &x_unit)
                        else {
                            excluded.record(&track_record_core::dsl::eval::EvalError::Data(
                                format!(
                                    "x is in {:?}, which cannot be converted to {x_unit:?}",
                                    out.signature.x_unit
                                ),
                            ));
                            continue;
                        };
                        if series.len() == max_series {
                            truncated = true;
                            break 'chunks;
                        }
                        series.push((c * CHUNK + j, points.scale_x(f)));
                    }
                    Err(e) => excluded.record(&e),
                }
            }
            tokio::task::yield_now().await;
        }
    }
    Ok(Loaded {
        prepared,
        ds,
        x_unit,
        series,
        excluded,
        truncated,
    })
}

fn grid_points(spec: Option<&GridSpec>) -> ApiResult<usize> {
    let points = spec.and_then(|g| g.points).unwrap_or(200);
    if !(2..=10_000).contains(&points) {
        return Err(ApiError::bad_request(
            "grid points must be between 2 and 10000",
        ));
    }
    Ok(points)
}

fn make_grid(spec: Option<&GridSpec>, series: &[&Series]) -> ApiResult<Vec<f64>> {
    let points = grid_points(spec)?;
    let range = extent(series.iter().copied());
    let from = spec.and_then(|g| g.from).or(range.map(|r| r.0));
    let to = spec.and_then(|g| g.to).or(range.map(|r| r.1));
    match (from, to) {
        (Some(a), Some(b)) if a.is_finite() && b.is_finite() && a <= b => Ok(grid(a, b, points)),
        (Some(_), Some(_)) => Err(ApiError::bad_request(
            "grid from must not be greater than to",
        )),
        _ => Ok(vec![]),
    }
}

pub async fn overlay(pool: &PgPool, req: &SeriesViewRequest) -> ApiResult<OverlayResponse> {
    let max_series = req.max_series.unwrap_or(500);
    if !(1..=5000).contains(&max_series) {
        return Err(ApiError::bad_request(
            "max_series must be between 1 and 5000",
        ));
    }
    let max_points = req.max_points.unwrap_or(2000);
    if max_points < 2 {
        return Err(ApiError::bad_request("max_points must be at least 2"));
    }
    grid_points(req.grid.as_ref())?;
    let l = load(pool, req, max_series).await?;
    let grid = match &req.grid {
        Some(spec) => {
            let all: Vec<&Series> = l.series.iter().map(|(_, s)| s).collect();
            Some(make_grid(Some(spec), &all)?)
        }
        None => None,
    };
    let mut cases = Cases::default();
    let mut out = Vec::new();
    for (i, s) in &l.series {
        let row = &l.ds.samples[*i];
        let tc = &l.ds.test_cases[&row.test_case_id];
        let key = case_key(tc, &[]);
        cases.add(&key, tc, &[]);
        let (x, y) = match &grid {
            Some(g) => (None, s.resample(g, req.interp.into())),
            None => {
                let d = s.downsample(max_points);
                (Some(d.x), d.y.into_iter().map(Some).collect())
            }
        };
        out.push(OverlaySeries {
            run: l.ds.runs[&row.run_id].clone(),
            case: cases.get(&key),
            client_device: row.client_device.clone(),
            case_execution_id: row.case_execution_id,
            sample_id: row.sample_id,
            repetition: row.repetition,
            is_warmup: row.is_warmup,
            x,
            y,
        });
    }
    Ok(OverlayResponse {
        metric: MetricInfo::of(&l.prepared),
        x_unit: l.x_unit,
        x_dimension: l.prepared.signature.x_dimension.clone(),
        grid,
        series: out,
        truncated: l.truncated,
        excluded: l.excluded,
    })
}

pub async fn aggregate(
    pool: &PgPool,
    req: &SeriesViewRequest,
) -> ApiResult<SeriesAggregateResponse> {
    let percentiles = req
        .percentiles
        .clone()
        .unwrap_or_else(|| vec![25.0, 50.0, 75.0]);
    if percentiles.iter().any(|p| !(0.0..=100.0).contains(p)) || percentiles.len() > 20 {
        return Err(ApiError::bad_request(
            "percentiles must be at most 20 values between 0 and 100",
        ));
    }
    grid_points(req.grid.as_ref())?;
    if req.max_series.is_some() || req.max_points.is_some() {
        return Err(ApiError::bad_request(
            "max_series and max_points apply to overlays",
        ));
    }
    let l = load(pool, req, usize::MAX).await?;
    let mut cases = Cases::default();
    let mut index: HashMap<(Uuid, String, String), usize> = HashMap::new();
    let mut groups: Vec<((Uuid, String, String), Vec<&Series>)> = Vec::new();
    for (i, s) in &l.series {
        let row = &l.ds.samples[*i];
        let tc = &l.ds.test_cases[&row.test_case_id];
        let key = case_key(tc, &[]);
        cases.add(&key, tc, &[]);
        let k = (row.run_id, key, row.client_device.clone());
        let at = *index.entry(k.clone()).or_insert_with(|| {
            groups.push((k, vec![]));
            groups.len() - 1
        });
        groups[at].1.push(s);
    }
    let mut out = Vec::new();
    for ((run, key, device), series) in groups {
        let x = make_grid(req.grid.as_ref(), &series)?;
        let rows: Vec<Vec<Option<f64>>> = series
            .iter()
            .map(|s| s.resample(&x, req.interp.into()))
            .collect();
        let band = aggregate_across(&rows, &percentiles);
        out.push(SeriesGroup {
            run: l.ds.runs[&run].clone(),
            case: cases.get(&key),
            client_device: device,
            n_series: series.len(),
            n: if x.is_empty() { vec![] } else { band.n },
            x,
            percentiles: percentiles
                .iter()
                .zip(band.percentiles)
                .map(|(p, values)| BandValues { p: *p, values })
                .collect(),
        });
    }
    out.sort_by(|a, b| {
        (
            a.run.started_at,
            a.run.id,
            case_sort_key(&a.case),
            &a.client_device,
        )
            .cmp(&(
                b.run.started_at,
                b.run.id,
                case_sort_key(&b.case),
                &b.client_device,
            ))
    });
    Ok(SeriesAggregateResponse {
        metric: MetricInfo::of(&l.prepared),
        x_unit: l.x_unit,
        x_dimension: l.prepared.signature.x_dimension.clone(),
        groups: out,
        excluded: l.excluded,
    })
}
