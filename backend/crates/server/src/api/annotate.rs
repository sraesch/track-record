//! Tag and annotation endpoints. Machine port with an `annotate` token, and
//! the UI port.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::error::{ApiJson, ApiPath, ApiResult, Problem};
use crate::service::annotations::{self, Annotation, CreateAnnotation, UpdateAnnotation};

/// Add a tag to a run. Adding an existing tag is a no-op.
#[utoipa::path(put, path = "/runs/{run_id}/tags/{tag}", tag = "annotations",
    params(("run_id" = Uuid, Path), ("tag" = String, Path)),
    responses((status = 204), (status = 404, body = Problem)))]
pub async fn add_run_tag(
    State(state): State<AppState>,
    ApiPath((run_id, tag)): ApiPath<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    annotations::add_run_tag(&state.pool, run_id, &tag).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a tag from a run.
#[utoipa::path(delete, path = "/runs/{run_id}/tags/{tag}", tag = "annotations",
    params(("run_id" = Uuid, Path), ("tag" = String, Path)),
    responses((status = 204), (status = 404, body = Problem)))]
pub async fn remove_run_tag(
    State(state): State<AppState>,
    ApiPath((run_id, tag)): ApiPath<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    annotations::remove_run_tag(&state.pool, run_id, &tag).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Add a tag to a test case.
#[utoipa::path(put, path = "/test-cases/{test_case_id}/tags/{tag}", tag = "annotations",
    params(("test_case_id" = i64, Path), ("tag" = String, Path)),
    responses((status = 204), (status = 404, body = Problem)))]
pub async fn add_test_case_tag(
    State(state): State<AppState>,
    ApiPath((id, tag)): ApiPath<(i64, String)>,
) -> ApiResult<StatusCode> {
    annotations::add_test_case_tag(&state.pool, id, &tag).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a tag from a test case.
#[utoipa::path(delete, path = "/test-cases/{test_case_id}/tags/{tag}", tag = "annotations",
    params(("test_case_id" = i64, Path), ("tag" = String, Path)),
    responses((status = 204), (status = 404, body = Problem)))]
pub async fn remove_test_case_tag(
    State(state): State<AppState>,
    ApiPath((id, tag)): ApiPath<(i64, String)>,
) -> ApiResult<StatusCode> {
    annotations::remove_test_case_tag(&state.pool, id, &tag).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Create an annotation on a run, a test case or a point in time.
#[utoipa::path(post, path = "/annotations", tag = "annotations",
    request_body = CreateAnnotation,
    responses((status = 201, body = Annotation), (status = 422, body = Problem)))]
pub async fn create_annotation(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CreateAnnotation>,
) -> ApiResult<(StatusCode, Json<Annotation>)> {
    Ok((
        StatusCode::CREATED,
        Json(annotations::create(&state.pool, &req).await?),
    ))
}

/// Edit an annotation's text.
#[utoipa::path(patch, path = "/annotations/{annotation_id}", tag = "annotations",
    params(("annotation_id" = Uuid, Path)),
    request_body = UpdateAnnotation,
    responses((status = 200, body = Annotation), (status = 404, body = Problem)))]
pub async fn update_annotation(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<UpdateAnnotation>,
) -> ApiResult<Json<Annotation>> {
    Ok(Json(annotations::update(&state.pool, id, &req).await?))
}

/// Delete an annotation.
#[utoipa::path(delete, path = "/annotations/{annotation_id}", tag = "annotations",
    params(("annotation_id" = Uuid, Path)),
    responses((status = 204), (status = 404, body = Problem)))]
pub async fn delete_annotation(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<StatusCode> {
    annotations::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
