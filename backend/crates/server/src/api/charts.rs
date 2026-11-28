//! Chart endpoints (FR-20, FR-22, FR-23, ADR 0022): run and check a spec
//! that is not saved. Machine port with any token scope, and the UI port.

use axum::Json;
use axum::extract::State;
use axum::response::Response;

use super::stream::{file, ndjson};
use crate::AppState;
use crate::error::{ApiJson, ApiResult, Problem};
use crate::service::charts::model::{ChartLine, ChartValidation, RunChart, ValidateChart};
use crate::service::charts::run::{self, RunOutput};
use crate::service::charts::validate;

/// Run a chart spec (FR-22, FR-23), streamed (NFR-7).
///
/// The response is NDJSON: a `start` event, `progress` events, one
/// `result` with the tree that reaches the visualization (or the node
/// `until` names), then `done`, or `error` if the run fails after it
/// started. With `format`, the response is instead the file of the data
/// that reaches the visualization (FR-20): text/csv, application/x-ndjson
/// or application/vnd.apache.parquet.
#[utoipa::path(post, path = "/charts/run", tag = "charts",
    request_body = RunChart,
    responses((status = 200,
               description = "NDJSON stream of events; with `format`, the file of the data that \
                              reaches the visualization: CSV, NDJSON with one object per row \
                              (not events) or Parquet",
               content(
                   (ChartLine = "application/x-ndjson"),
                   (String = "text/csv"),
                   (String = "application/vnd.apache.parquet"))),
              (status = 400, content_type = "application/problem+json", body = Problem),
              (status = 422, content_type = "application/problem+json", body = Problem)))]
pub async fn run_chart(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<RunChart>,
) -> ApiResult<Response> {
    Ok(
        match run::run(&state.pool, state.chart_limits, req).await? {
            RunOutput::Events(rx) => ndjson(rx),
            RunOutput::File { stream, format } => file(stream, format, "track-record-chart"),
        },
    )
}

/// Check a chart spec: whether it is well-formed and valid, its problems
/// per node and setting, and the columns, attributes and group levels of
/// each node (ADR 0022, "Spec"). Problems are part of the answer.
#[utoipa::path(post, path = "/charts/validate", tag = "charts",
    request_body = ValidateChart,
    responses((status = 200, body = ChartValidation),
              (status = 400, content_type = "application/problem+json", body = Problem),
              (status = 422, content_type = "application/problem+json", body = Problem)))]
pub async fn validate_chart(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<ValidateChart>,
) -> ApiResult<Json<ChartValidation>> {
    Ok(Json(validate::validate(&state.pool, &req).await?))
}
