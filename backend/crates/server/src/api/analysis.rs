//! Derived metric and analysis endpoints. Machine port with any token scope,
//! and the UI port. They only read; saving a derived metric is in `ui.rs`.
//!
//! Long operations stream (NFR-7): the comparison as NDJSON events, the
//! export as the file itself.

use axum::Json;
use axum::extract::State;
use axum::response::Response;

use super::stream::{file, ndjson};
use crate::AppState;
use crate::error::{ApiJson, ApiPath, ApiResult, Problem};
use crate::service::analysis::compare::{self, CompareEvent, CompareRequest};
use crate::service::analysis::export::{self, ExportRequest};
use crate::service::analysis::series::{
    self, OverlayResponse, SeriesAggregateResponse, SeriesViewRequest,
};
use crate::service::analysis::{
    self as analysis, AggregatesRequest, AggregatesResponse, TrendRequest, TrendResponse,
};
use crate::service::derived::{
    self, DerivedMetric, EvaluateRequest, EvaluateResponse, ValidateExpression, Validation,
};

/// Derived metrics (FR-17).
#[utoipa::path(get, path = "/derived-metrics", tag = "definitions",
    responses((status = 200, body = Vec<DerivedMetric>)))]
pub async fn list_derived_metrics(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<DerivedMetric>>> {
    Ok(Json(derived::list(&state.pool).await?))
}

/// A derived metric.
#[utoipa::path(get, path = "/derived-metrics/{name}", tag = "definitions",
    params(("name" = String, Path)),
    responses((status = 200, body = DerivedMetric), (status = 404, body = Problem)))]
pub async fn get_derived_metric(
    State(state): State<AppState>,
    ApiPath(name): ApiPath<String>,
) -> ApiResult<Json<DerivedMetric>> {
    Ok(Json(derived::get(&state.pool, &name).await?))
}

/// Validate a DSL expression (FR-17a): syntax, references and units.
///
/// Always 200; `valid` says whether it passed, `diagnostics` point at the
/// problems by byte offset.
#[utoipa::path(post, path = "/derived-metrics/validate", tag = "definitions",
    request_body = ValidateExpression,
    responses((status = 200, body = Validation)))]
pub async fn validate_expression(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<ValidateExpression>,
) -> ApiResult<Json<Validation>> {
    Ok(Json(derived::validate(&state.pool, &req).await?))
}

/// Evaluate an expression or a saved derived metric on the samples of a
/// case execution.
#[utoipa::path(post, path = "/derived-metrics/evaluate", tag = "definitions",
    request_body = EvaluateRequest,
    responses((status = 200, body = EvaluateResponse), (status = 404, body = Problem),
              (status = 422, body = Problem)))]
pub async fn evaluate_expression(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<EvaluateRequest>,
) -> ApiResult<Json<EvaluateResponse>> {
    Ok(Json(derived::evaluate(&state.pool, &req).await?))
}

/// Aggregates across samples per run, case and client device (FR-12).
#[utoipa::path(post, path = "/analysis/aggregates", tag = "analysis",
    request_body = AggregatesRequest,
    responses((status = 200, body = AggregatesResponse), (status = 400, body = Problem),
              (status = 422, body = Problem)))]
pub async fn aggregates(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<AggregatesRequest>,
) -> ApiResult<Json<AggregatesResponse>> {
    Ok(Json(analysis::aggregates(&state.pool, &req).await?))
}

/// Trend of a case, metric and client device across runs (FR-18).
#[utoipa::path(post, path = "/analysis/trend", tag = "analysis",
    request_body = TrendRequest,
    responses((status = 200, body = TrendResponse), (status = 404, body = Problem),
              (status = 422, body = Problem)))]
pub async fn trend(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<TrendRequest>,
) -> ApiResult<Json<TrendResponse>> {
    Ok(Json(analysis::trend(&state.pool, &req).await?))
}

/// Compare side by side along a dimension (FR-16), streamed (NFR-7).
///
/// The response is NDJSON: a `start` event, `progress` events, one `row`
/// event per matching key, and `done` (or `error`) last.
#[utoipa::path(post, path = "/analysis/compare", tag = "analysis",
    request_body = CompareRequest,
    responses((status = 200, description = "NDJSON stream of events",
               content_type = "application/x-ndjson", body = CompareEvent),
              (status = 400, body = Problem), (status = 422, body = Problem)))]
pub async fn compare(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CompareRequest>,
) -> ApiResult<Response> {
    Ok(ndjson(compare::compare(state.pool.clone(), req).await?))
}

/// Overlay series of many samples, optionally resampled onto a common grid
/// (FR-19).
#[utoipa::path(post, path = "/analysis/series/overlay", tag = "analysis",
    request_body = SeriesViewRequest,
    responses((status = 200, body = OverlayResponse), (status = 400, body = Problem),
              (status = 422, body = Problem)))]
pub async fn series_overlay(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<SeriesViewRequest>,
) -> ApiResult<Json<OverlayResponse>> {
    Ok(Json(series::overlay(&state.pool, &req).await?))
}

/// Aggregate series across samples per run, case and device: percentiles
/// per grid point, e.g. a median with a band (FR-19).
#[utoipa::path(post, path = "/analysis/series/aggregate", tag = "analysis",
    request_body = SeriesViewRequest,
    responses((status = 200, body = SeriesAggregateResponse), (status = 400, body = Problem),
              (status = 422, body = Problem)))]
pub async fn series_aggregate(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<SeriesViewRequest>,
) -> ApiResult<Json<SeriesAggregateResponse>> {
    Ok(Json(series::aggregate(&state.pool, &req).await?))
}

/// Export per-sample values or aggregates as CSV, NDJSON or Parquet
/// (FR-20), streamed.
#[utoipa::path(post, path = "/analysis/export", tag = "analysis",
    request_body = ExportRequest,
    responses((status = 200, description = "The file (text/csv, application/x-ndjson or \
                                            application/vnd.apache.parquet)",
               content_type = "application/octet-stream"),
              (status = 400, body = Problem), (status = 422, body = Problem)))]
pub async fn export(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<ExportRequest>,
) -> ApiResult<Response> {
    let format = req.format;
    let rx = export::export(state.pool.clone(), req).await?;
    Ok(file(rx, format, "track-record-export"))
}
