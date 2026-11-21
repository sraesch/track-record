//! API errors as RFC 9457 problem details, and the mapping of database errors
//! (the custom SQLSTATEs TR001–TR007 from db/README.md and the standard ones)
//! to HTTP status codes.

use axum::extract::FromRequest;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// Problem details (RFC 9457). `code` is a stable, machine-readable error
/// code.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Problem {
    /// Short summary of the problem type.
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// Stable error code, e.g. `conflict` or `series_incomplete`.
    pub code: String,
    /// Explanation of this occurrence.
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{detail}")]
    Problem {
        status: StatusCode,
        code: &'static str,
        detail: String,
    },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, detail: impl Into<String>) -> Self {
        Self::Problem {
            status,
            code,
            detail: detail.into(),
        }
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", detail)
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", detail)
    }

    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
            detail,
        )
    }

    /// A retried upload whose content differs from what is stored (FR-3, FR-6).
    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", detail)
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", detail)
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::Problem { status, .. } => *status,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Problem { code, .. } => code,
            Self::Internal(_) => "internal",
        }
    }
}

/// Maps a SQLSTATE to status and error code, or `None` for unexpected errors.
pub fn map_sqlstate(code: &str) -> Option<(StatusCode, &'static str)> {
    Some(match code {
        "TR001" => (StatusCode::CONFLICT, "immutable"),
        "TR002" => (StatusCode::CONFLICT, "delete_not_allowed"),
        "TR003" => (StatusCode::CONFLICT, "invalid_status_transition"),
        "TR004" => (StatusCode::CONFLICT, "run_not_running"),
        "TR005" => (StatusCode::UNPROCESSABLE_ENTITY, "series_incomplete"),
        "TR006" => (StatusCode::UNPROCESSABLE_ENTITY, "series_x_decreasing"),
        "TR007" => (StatusCode::UNPROCESSABLE_ENTITY, "derived_metric_cycle"),
        "23505" => (StatusCode::CONFLICT, "duplicate"),
        "23503" => (StatusCode::UNPROCESSABLE_ENTITY, "reference_not_found"),
        "23514" | "23502" => (StatusCode::UNPROCESSABLE_ENTITY, "check_violation"),
        "22P02" | "22023" | "22007" | "22008" | "22003" => {
            (StatusCode::UNPROCESSABLE_ENTITY, "invalid_value")
        }
        _ => return None,
    })
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err
            && let Some(code) = db.code()
            && let Some((status, code)) = map_sqlstate(&code)
        {
            return Self::new(status, code, db.message().to_owned());
        }
        if matches!(err, sqlx::Error::RowNotFound) {
            return Self::not_found("not found");
        }
        Self::Internal(err.into())
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        let status = match rejection {
            JsonRejection::MissingJsonContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            JsonRejection::JsonSyntaxError(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        Self::new(status, "invalid_request_body", rejection.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_path",
            rejection.body_text(),
        )
    }
}

impl From<axum_extra::extract::QueryRejection> for ApiError {
    fn from(rejection: axum_extra::extract::QueryRejection) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            rejection.body_text(),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let detail = match &self {
            Self::Problem { detail, .. } => detail.clone(),
            Self::Internal(err) => {
                tracing::error!(error = ?err, "internal error");
                "internal server error".to_owned()
            }
        };
        let problem = Problem {
            title: status.canonical_reason().unwrap_or("Error").to_owned(),
            status: status.as_u16(),
            code: self.code().to_owned(),
            detail,
        };
        let mut response = (status, axum::Json(problem)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

/// JSON body extractor whose errors are problem details (FR-4).
#[derive(Debug, FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

/// Path extractor whose errors are problem details.
#[derive(Debug, axum::extract::FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct ApiPath<T>(pub T);

/// Query extractor (repeated keys allowed) whose errors are problem details.
#[derive(Debug, axum::extract::FromRequestParts)]
#[from_request(via(axum_extra::extract::Query), rejection(ApiError))]
pub struct ApiQuery<T>(pub T);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_sqlstates_map_to_http_statuses() {
        assert_eq!(
            map_sqlstate("TR001"),
            Some((StatusCode::CONFLICT, "immutable"))
        );
        assert_eq!(
            map_sqlstate("TR004"),
            Some((StatusCode::CONFLICT, "run_not_running"))
        );
        assert_eq!(
            map_sqlstate("TR005"),
            Some((StatusCode::UNPROCESSABLE_ENTITY, "series_incomplete"))
        );
        assert_eq!(
            map_sqlstate("TR007"),
            Some((StatusCode::UNPROCESSABLE_ENTITY, "derived_metric_cycle"))
        );
        assert_eq!(
            map_sqlstate("23505"),
            Some((StatusCode::CONFLICT, "duplicate"))
        );
        assert_eq!(map_sqlstate("40001"), None);
    }

    #[tokio::test]
    async fn problem_response_has_problem_json_content_type() {
        let response = ApiError::conflict("differs").into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        let body = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["code"], "conflict");
        assert_eq!(value["status"], 409);
        assert_eq!(value["detail"], "differs");
    }

    #[tokio::test]
    async fn internal_errors_hide_details() {
        let response = ApiError::Internal(anyhow::anyhow!("secret")).into_response();
        let body = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("secret"));
    }
}
