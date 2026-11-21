//! API token management (NFR-2), only on the UI port. The secret is shown
//! once at creation; only its SHA-256 is stored.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{generate_token, hash_token};
use crate::db::TokenScope;
use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateToken {
    pub name: String,
    pub scope: TokenScope,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ApiToken {
    pub id: Uuid,
    pub name: String,
    pub scope: TokenScope,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreatedToken {
    #[serde(flatten)]
    pub token: ApiToken,
    /// The secret. Shown only once.
    pub secret: String,
}

const TOKEN_SELECT: &str =
    "SELECT id, name, scope, created_at, last_used_at, revoked_at FROM api_token";

pub async fn create(pool: &PgPool, req: &CreateToken) -> ApiResult<CreatedToken> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::invalid("token name must not be empty"));
    }
    let secret = generate_token();
    let token: ApiToken = sqlx::query_as(
        "INSERT INTO api_token (name, scope, token_hash) VALUES ($1, $2, $3) \
         RETURNING id, name, scope, created_at, last_used_at, revoked_at",
    )
    .bind(name)
    .bind(req.scope)
    .bind(hash_token(&secret))
    .fetch_one(pool)
    .await?;
    Ok(CreatedToken { token, secret })
}

pub async fn list(pool: &PgPool) -> ApiResult<Vec<ApiToken>> {
    Ok(
        sqlx::query_as(&format!("{TOKEN_SELECT} ORDER BY created_at DESC, id"))
            .fetch_all(pool)
            .await?,
    )
}

/// Revokes a token. Revoking it again is a no-op. Revoked tokens are kept.
pub async fn revoke(pool: &PgPool, id: Uuid) -> ApiResult<ApiToken> {
    sqlx::query("UPDATE api_token SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query_as(&format!("{TOKEN_SELECT} WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("token {id} not found")))
}
