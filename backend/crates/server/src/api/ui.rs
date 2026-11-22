//! Endpoints that exist only on the UI port (behind the auth proxy): run
//! invalidation and hard delete, token management, alias and derived
//! metric editing.
//! No API token can reach them (NFR-2).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::error::{ApiJson, ApiPath, ApiResult, Problem};
use crate::service::catalog::{self, Alias, SetAlias};
use crate::service::derived::{self, SaveDerivedMetric, SavedDerivedMetric};
use crate::service::lifecycle::{self, DeleteRun, InvalidateRun};
use crate::service::tokens::{self, ApiToken, CreateToken, CreatedToken};

/// Mark a run invalid with a reason (FR-9).
#[utoipa::path(post, path = "/runs/{run_id}/invalidate", tag = "ui",
    params(("run_id" = Uuid, Path)),
    request_body = InvalidateRun,
    responses((status = 204), (status = 404, body = Problem), (status = 422, body = Problem)))]
pub async fn invalidate_run(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<InvalidateRun>,
) -> ApiResult<StatusCode> {
    lifecycle::invalidate(&state.pool, run_id, &req).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Mark a run valid again (FR-9).
#[utoipa::path(post, path = "/runs/{run_id}/validate", tag = "ui",
    params(("run_id" = Uuid, Path)),
    responses((status = 204), (status = 404, body = Problem)))]
pub async fn validate_run(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
) -> ApiResult<StatusCode> {
    lifecycle::validate(&state.pool, run_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Hard-delete a run with all results and attachments (FR-10).
///
/// `confirm_run_id` must repeat the run id. Cannot be undone; deleted runs
/// can only be restored from backup.
#[utoipa::path(post, path = "/runs/{run_id}/delete", tag = "ui",
    params(("run_id" = Uuid, Path)),
    request_body = DeleteRun,
    responses((status = 204), (status = 400, description = "Confirmation does not match", body = Problem),
              (status = 404, body = Problem)))]
pub async fn delete_run(
    State(state): State<AppState>,
    ApiPath(run_id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<DeleteRun>,
) -> ApiResult<StatusCode> {
    lifecycle::delete(&state, run_id, &req).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// API tokens (without secrets).
#[utoipa::path(get, path = "/tokens", tag = "ui", responses((status = 200, body = Vec<ApiToken>)))]
pub async fn list_tokens(State(state): State<AppState>) -> ApiResult<Json<Vec<ApiToken>>> {
    Ok(Json(tokens::list(&state.pool).await?))
}

/// Create an API token. The secret is in this response only.
#[utoipa::path(post, path = "/tokens", tag = "ui", request_body = CreateToken,
    responses((status = 201, body = CreatedToken), (status = 422, body = Problem)))]
pub async fn create_token(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CreateToken>,
) -> ApiResult<(StatusCode, Json<CreatedToken>)> {
    Ok((
        StatusCode::CREATED,
        Json(tokens::create(&state.pool, &req).await?),
    ))
}

/// Revoke an API token.
#[utoipa::path(post, path = "/tokens/{token_id}/revoke", tag = "ui",
    params(("token_id" = Uuid, Path)),
    responses((status = 200, body = ApiToken), (status = 404, body = Problem)))]
pub async fn revoke_token(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Json<ApiToken>> {
    Ok(Json(tokens::revoke(&state.pool, id).await?))
}

/// Set or remove (`target: null`) an alias (FR-14). Each change is a new
/// version.
#[utoipa::path(put, path = "/aliases", tag = "ui", request_body = SetAlias,
    responses((status = 200, description = "The alias; null if a missing alias was removed",
               body = Option<Alias>),
              (status = 422, body = Problem)))]
pub async fn set_alias(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<SetAlias>,
) -> ApiResult<Json<Option<Alias>>> {
    Ok(Json(catalog::set_alias(&state.pool, &req).await?))
}

/// Create a derived metric or change its definition (FR-17). A derived
/// metric has no versions: the change applies to every analysis, historical
/// runs included, and to the derived metrics built on it. The expression is
/// validated first (FR-17a), together with those metrics.
#[utoipa::path(put, path = "/derived-metrics/{name}", tag = "ui",
    params(("name" = String, Path)),
    request_body = SaveDerivedMetric,
    responses((status = 201, description = "Created", body = SavedDerivedMetric),
              (status = 200, description = "Changed, or saved again unchanged",
               body = SavedDerivedMetric),
              (status = 422, description = "Invalid expression, a cycle, or a change that \
                                            breaks a derived metric built on this one",
               body = Problem)))]
pub async fn save_derived_metric(
    State(state): State<AppState>,
    ApiPath(name): ApiPath<String>,
    ApiJson(req): ApiJson<SaveDerivedMetric>,
) -> ApiResult<(StatusCode, Json<SavedDerivedMetric>)> {
    let saved = derived::save(&state.pool, &name, &req).await?;
    let status = if saved.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(saved)))
}
