//! Ingestion endpoints (FR-1 to FR-5). Machine port, `write` scope.

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use uuid::Uuid;

use crate::AppState;
use crate::error::{ApiJson, ApiPath, ApiQuery, ApiResult, Problem};
use crate::service::attachments::{self, UploadAttachment, UploadHeaders};
use crate::service::ingest::{
    self, CloseRun, CreateCaseExecution, CreateRun, CreateSample, CreateSeries, IngestResponse,
};

type Created = (StatusCode, Json<IngestResponse>);

fn created(id: Uuid, created: bool) -> Created {
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    (status, Json(IngestResponse { id, created }))
}

/// Create a run.
///
/// Idempotent: the same content again returns 200, different content 409.
#[utoipa::path(put, path = "/runs/{run_id}", tag = "ingestion",
    params(("run_id" = Uuid, Path, description = "Client-generated run id")),
    request_body = CreateRun,
    responses(
        (status = 201, description = "Run created", body = IngestResponse),
        (status = 200, description = "Run already existed with this content", body = IngestResponse),
        (status = 409, description = "Run exists with other content", body = Problem),
        (status = 422, description = "Invalid request", body = Problem),
    ))]
pub async fn create_run(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<CreateRun>,
) -> ApiResult<Created> {
    let mut tx = state.pool.begin().await?;
    let was_created = ingest::create_run(&mut tx, run_id, &req).await?;
    tx.commit().await?;
    Ok(created(run_id, was_created))
}

/// Close a run with its final status.
///
/// Sending the same status again is a no-op. A run that was aborted for
/// inactivity cannot be closed (409).
#[utoipa::path(post, path = "/runs/{run_id}/close", tag = "ingestion",
    params(("run_id" = Uuid, Path)),
    request_body = CloseRun,
    responses(
        (status = 200, body = IngestResponse),
        (status = 404, body = Problem),
        (status = 409, description = "Run already has another terminal status", body = Problem),
    ))]
pub async fn close_run(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<CloseRun>,
) -> ApiResult<Json<IngestResponse>> {
    let mut conn = state.pool.acquire().await?;
    let changed = ingest::close_run(&mut conn, run_id, &req).await?;
    Ok(Json(IngestResponse {
        id: run_id,
        created: changed,
    }))
}

/// Add a case execution, optionally with its samples and series.
///
/// The whole request is one transaction. New case executions need a running
/// run; corrections (`supersedes_id`) may be added to finished runs.
#[utoipa::path(put, path = "/runs/{run_id}/case-executions/{case_execution_id}", tag = "ingestion",
    params(("run_id" = Uuid, Path), ("case_execution_id" = Uuid, Path)),
    request_body = CreateCaseExecution,
    responses(
        (status = 201, body = IngestResponse),
        (status = 200, description = "Everything already existed", body = IngestResponse),
        (status = 404, body = Problem),
        (status = 409, description = "Conflicting content, or the run is not running", body = Problem),
        (status = 422, body = Problem),
    ))]
pub async fn create_case_execution(
    State(state): State<AppState>,
    ApiPath((run_id, ce_id)): ApiPath<(Uuid, Uuid)>,
    ApiJson(req): ApiJson<CreateCaseExecution>,
) -> ApiResult<Created> {
    let mut tx = state.pool.begin().await?;
    let was_created = ingest::create_case_execution(&mut tx, run_id, ce_id, &req).await?;
    tx.commit().await?;
    Ok(created(ce_id, was_created))
}

/// Add a sample, optionally with its series.
#[utoipa::path(put,
    path = "/runs/{run_id}/case-executions/{case_execution_id}/samples/{sample_id}",
    tag = "ingestion",
    params(("run_id" = Uuid, Path), ("case_execution_id" = Uuid, Path), ("sample_id" = Uuid, Path)),
    request_body = CreateSample,
    responses(
        (status = 201, body = IngestResponse),
        (status = 200, body = IngestResponse),
        (status = 404, body = Problem),
        (status = 409, body = Problem),
        (status = 422, body = Problem),
    ))]
pub async fn create_sample(
    State(state): State<AppState>,
    ApiPath((run_id, ce_id, sample_id)): ApiPath<(Uuid, Uuid, Uuid)>,
    ApiJson(req): ApiJson<CreateSample>,
) -> ApiResult<Created> {
    let mut tx = state.pool.begin().await?;
    let was_created = ingest::create_sample(&mut tx, run_id, ce_id, sample_id, &req).await?;
    tx.commit().await?;
    Ok(created(sample_id, was_created))
}

/// Add a series to a sample. The series and all points are one transaction.
#[utoipa::path(put,
    path = "/runs/{run_id}/case-executions/{case_execution_id}/samples/{sample_id}/series/{series_id}",
    tag = "ingestion",
    params(("run_id" = Uuid, Path), ("case_execution_id" = Uuid, Path),
           ("sample_id" = Uuid, Path), ("series_id" = Uuid, Path)),
    request_body = CreateSeries,
    responses(
        (status = 201, body = IngestResponse),
        (status = 200, body = IngestResponse),
        (status = 404, body = Problem),
        (status = 409, body = Problem),
        (status = 422, body = Problem),
    ))]
pub async fn create_series(
    State(state): State<AppState>,
    ApiPath((run_id, ce_id, sample_id, series_id)): ApiPath<(Uuid, Uuid, Uuid, Uuid)>,
    ApiJson(req): ApiJson<CreateSeries>,
) -> ApiResult<Created> {
    let mut tx = state.pool.begin().await?;
    let was_created =
        ingest::create_series(&mut tx, run_id, ce_id, sample_id, series_id, &req).await?;
    tx.commit().await?;
    Ok(created(series_id, was_created))
}

/// Upload an attachment (raw body).
///
/// `Content-Type` is stored for download. With `X-Content-SHA256` (hex) the
/// body is checked. Same id and content again returns 200.
#[utoipa::path(put, path = "/runs/{run_id}/attachments/{attachment_id}", tag = "ingestion",
    params(("run_id" = Uuid, Path), ("attachment_id" = Uuid, Path), UploadAttachment,
           ("X-Content-SHA256" = Option<String>, Header, description = "Expected SHA-256, hex")),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 201, body = IngestResponse),
        (status = 200, body = IngestResponse),
        (status = 404, body = Problem),
        (status = 409, body = Problem),
        (status = 422, body = Problem),
    ))]
pub async fn upload_attachment(
    State(state): State<AppState>,
    ApiPath((run_id, attachment_id)): ApiPath<(Uuid, Uuid)>,
    ApiQuery(params): ApiQuery<UploadAttachment>,
    headers: HeaderMap,
    body: Body,
) -> ApiResult<Created> {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let upload_headers = UploadHeaders {
        content_type: header(header::CONTENT_TYPE.as_str())
            .unwrap_or_else(|| "application/octet-stream".to_owned()),
        expected_sha256: header("x-content-sha256"),
    };
    let was_created = attachments::upload(
        &state,
        run_id,
        attachment_id,
        &params,
        &upload_headers,
        body.into_data_stream(),
    )
    .await?;
    Ok(created(attachment_id, was_created))
}
