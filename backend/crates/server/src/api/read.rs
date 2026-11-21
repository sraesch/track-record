//! Read endpoints. Machine port with any token scope, and the UI port.

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::AppState;
use crate::error::{ApiError, ApiPath, ApiQuery, ApiResult, Problem};
use crate::service::annotations::{self, Annotation, AnnotationQuery};
use crate::service::catalog::{
    self, Alias, Asset, ClientDevice, Environment, Metric, TestCaseQuery, Unit, VersionedDefinition,
};
use crate::service::runs::{
    self, AttachmentMeta, CaseExecutionDetail, CaseExecutionListQuery, CaseExecutionSummary,
    RunDetail, RunFilter, RunList, SeriesMeta, SeriesPoints, SeriesPointsQuery, SutVersion,
    TestCaseSummary,
};

/// List runs, newest measured start first (FR-11).
#[utoipa::path(get, path = "/runs", tag = "runs", params(RunFilter),
    responses((status = 200, body = RunList), (status = 400, body = Problem)))]
pub async fn list_runs(
    State(state): State<AppState>,
    ApiQuery(filter): ApiQuery<RunFilter>,
) -> ApiResult<Json<RunList>> {
    Ok(Json(runs::list_runs(&state.pool, &filter).await?))
}

/// Run detail with tags, annotations and attachments.
#[utoipa::path(get, path = "/runs/{run_id}", tag = "runs", params(("run_id" = Uuid, Path)),
    responses((status = 200, body = RunDetail), (status = 404, body = Problem)))]
pub async fn get_run(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
) -> ApiResult<Json<RunDetail>> {
    Ok(Json(runs::get_run(&state.pool, run_id).await?))
}

/// Case executions of a run. Superseded ones are hidden by default.
#[utoipa::path(get, path = "/runs/{run_id}/case-executions", tag = "runs",
    params(("run_id" = Uuid, Path), CaseExecutionListQuery),
    responses((status = 200, body = Vec<CaseExecutionSummary>), (status = 404, body = Problem)))]
pub async fn list_case_executions(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<CaseExecutionListQuery>,
) -> ApiResult<Json<Vec<CaseExecutionSummary>>> {
    Ok(Json(
        runs::list_case_executions(&state.pool, run_id, &query).await?,
    ))
}

/// Attachments of a run.
#[utoipa::path(get, path = "/runs/{run_id}/attachments", tag = "runs",
    params(("run_id" = Uuid, Path)),
    responses((status = 200, body = Vec<AttachmentMeta>)))]
pub async fn list_attachments(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
) -> ApiResult<Json<Vec<AttachmentMeta>>> {
    Ok(Json(runs::list_attachments(&state.pool, run_id).await?))
}

/// Case execution with samples, measurements and series metadata.
#[utoipa::path(get, path = "/case-executions/{case_execution_id}", tag = "runs",
    params(("case_execution_id" = Uuid, Path)),
    responses((status = 200, body = CaseExecutionDetail), (status = 404, body = Problem)))]
pub async fn get_case_execution(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Json<CaseExecutionDetail>> {
    Ok(Json(runs::get_case_execution(&state.pool, id).await?))
}

/// Series metadata.
#[utoipa::path(get, path = "/series/{series_id}", tag = "runs",
    params(("series_id" = Uuid, Path)),
    responses((status = 200, body = SeriesMeta), (status = 404, body = Problem)))]
pub async fn get_series(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Json<SeriesMeta>> {
    Ok(Json(runs::get_series(&state.pool, id).await?))
}

/// Points of a series, optionally downsampled for display (FR-19).
#[utoipa::path(get, path = "/series/{series_id}/points", tag = "runs",
    params(("series_id" = Uuid, Path), SeriesPointsQuery),
    responses((status = 200, body = SeriesPoints), (status = 400, body = Problem),
              (status = 404, body = Problem)))]
pub async fn get_series_points(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<SeriesPointsQuery>,
) -> ApiResult<Json<SeriesPoints>> {
    Ok(Json(
        runs::get_series_points(&state.pool, id, &query).await?,
    ))
}

/// Attachment metadata.
#[utoipa::path(get, path = "/attachments/{attachment_id}", tag = "runs",
    params(("attachment_id" = Uuid, Path)),
    responses((status = 200, body = AttachmentMeta), (status = 404, body = Problem)))]
pub async fn get_attachment(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Json<AttachmentMeta>> {
    Ok(Json(runs::get_attachment(&state.pool, id).await?))
}

/// `Content-Disposition: attachment` with an ASCII fallback and the UTF-8
/// name (RFC 6266). Uploaded files are never rendered inline.
fn content_disposition(file_name: &str) -> HeaderValue {
    let ascii: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded = utf8_percent_encode(file_name, NON_ALPHANUMERIC);
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// Download an attachment.
#[utoipa::path(get, path = "/attachments/{attachment_id}/content", tag = "runs",
    params(("attachment_id" = Uuid, Path)),
    responses((status = 200, description = "The file", content_type = "application/octet-stream"),
              (status = 404, body = Problem)))]
pub async fn download_attachment(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Response> {
    let meta = runs::get_attachment(&state.pool, id).await?;
    let file = state
        .attachments
        .open(meta.run_id, meta.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("attachment {id} file missing: {e}")))?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&meta.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(meta.size_bytes));
    headers.insert(
        header::CONTENT_DISPOSITION,
        content_disposition(&meta.file_name),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox"),
    );
    Ok(response)
}

/// Suites with all versions.
#[utoipa::path(get, path = "/suites", tag = "definitions",
    responses((status = 200, body = Vec<VersionedDefinition>)))]
pub async fn list_suites(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<VersionedDefinition>>> {
    Ok(Json(catalog::suites(&state.pool).await?))
}

/// Scenarios with all versions.
#[utoipa::path(get, path = "/scenarios", tag = "definitions",
    responses((status = 200, body = Vec<VersionedDefinition>)))]
pub async fn list_scenarios(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<VersionedDefinition>>> {
    Ok(Json(catalog::scenarios(&state.pool).await?))
}

/// Metrics with all versions.
#[utoipa::path(get, path = "/metrics", tag = "definitions",
    responses((status = 200, body = Vec<Metric>)))]
pub async fn list_metrics(State(state): State<AppState>) -> ApiResult<Json<Vec<Metric>>> {
    Ok(Json(catalog::metrics(&state.pool).await?))
}

/// The predefined unit catalogue (FR-13a).
#[utoipa::path(get, path = "/units", tag = "definitions",
    responses((status = 200, body = Vec<Unit>)))]
pub async fn list_units(State(state): State<AppState>) -> ApiResult<Json<Vec<Unit>>> {
    Ok(Json(catalog::units(&state.pool).await?))
}

/// Client devices with their names after aliases.
#[utoipa::path(get, path = "/client-devices", tag = "definitions",
    responses((status = 200, body = Vec<ClientDevice>)))]
pub async fn list_client_devices(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<ClientDevice>>> {
    Ok(Json(catalog::client_devices(&state.pool).await?))
}

/// An environment (shared environment or device details).
#[utoipa::path(get, path = "/environments/{environment_id}", tag = "definitions",
    params(("environment_id" = i64, Path)),
    responses((status = 200, body = Environment), (status = 404, body = Problem)))]
pub async fn get_environment(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Environment>> {
    Ok(Json(catalog::environment(&state.pool, id).await?))
}

/// SUT versions, newest first.
#[utoipa::path(get, path = "/sut-versions", tag = "definitions",
    responses((status = 200, body = Vec<SutVersion>)))]
pub async fn list_sut_versions(State(state): State<AppState>) -> ApiResult<Json<Vec<SutVersion>>> {
    Ok(Json(catalog::sut_versions(&state.pool).await?))
}

/// Assets.
#[utoipa::path(get, path = "/assets", tag = "definitions",
    responses((status = 200, body = Vec<Asset>)))]
pub async fn list_assets(State(state): State<AppState>) -> ApiResult<Json<Vec<Asset>>> {
    Ok(Json(catalog::assets(&state.pool).await?))
}

/// Test cases (scenario version + assets + parameters).
#[utoipa::path(get, path = "/test-cases", tag = "definitions", params(TestCaseQuery),
    responses((status = 200, body = Vec<TestCaseSummary>)))]
pub async fn list_test_cases(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<TestCaseQuery>,
) -> ApiResult<Json<Vec<TestCaseSummary>>> {
    Ok(Json(catalog::test_cases(&state.pool, &query).await?))
}

/// A test case.
#[utoipa::path(get, path = "/test-cases/{test_case_id}", tag = "definitions",
    params(("test_case_id" = i64, Path)),
    responses((status = 200, body = TestCaseSummary), (status = 404, body = Problem)))]
pub async fn get_test_case(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<TestCaseSummary>> {
    Ok(Json(catalog::test_case(&state.pool, id).await?))
}

/// Aliases with their current target (FR-14).
#[utoipa::path(get, path = "/aliases", tag = "definitions",
    responses((status = 200, body = Vec<Alias>)))]
pub async fn list_aliases(State(state): State<AppState>) -> ApiResult<Json<Vec<Alias>>> {
    Ok(Json(catalog::aliases(&state.pool).await?))
}

/// Annotations, filtered by run, test case or time range.
#[utoipa::path(get, path = "/annotations", tag = "annotations", params(AnnotationQuery),
    responses((status = 200, body = Vec<Annotation>)))]
pub async fn list_annotations(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<AnnotationQuery>,
) -> ApiResult<Json<Vec<Annotation>>> {
    Ok(Json(annotations::list(&state.pool, &query).await?))
}

/// An annotation.
#[utoipa::path(get, path = "/annotations/{annotation_id}", tag = "annotations",
    params(("annotation_id" = Uuid, Path)),
    responses((status = 200, body = Annotation), (status = 404, body = Problem)))]
pub async fn get_annotation(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Json<Annotation>> {
    Ok(Json(annotations::get(&state.pool, id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_disposition_never_inlines_and_escapes_names() {
        let v = content_disposition("report \"final\".html");
        let s = v.to_str().unwrap();
        assert!(s.starts_with("attachment; filename=\"report _final_.html\""));
        assert!(s.contains("filename*=UTF-8''report%20%22final%22%2Ehtml"));
        let v = content_disposition("größe.txt");
        assert!(v.to_str().unwrap().contains("filename=\"gr__e.txt\""));
    }
}
