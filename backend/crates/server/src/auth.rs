//! API token authentication for the machine port (NFR-2).
//!
//! Tokens look like `trk_<43 base64url characters>` (32 random bytes). Only
//! their SHA-256 is stored. The UI port has no token check: it sits behind
//! the auth proxy and has no user identity.

use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AppState;
use crate::db::TokenScope;
use crate::error::ApiError;

pub const TOKEN_PREFIX: &str = "trk_";

/// A fresh random token secret.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("operating system random number generator failed");
    format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// The authenticated token of a machine request, stored in the request
/// extensions.
#[derive(Debug, Clone, Copy)]
pub struct Authenticated {
    pub token_id: Uuid,
    pub scope: TokenScope,
}

fn bearer(request: &Request) -> Option<&str> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Looks up an active token. Updates `last_used_at` at most once a minute,
/// so reads don't turn into a write per request.
pub async fn authenticate(state: &AppState, token: &str) -> Result<Authenticated, ApiError> {
    let row: Option<(Uuid, TokenScope)> = sqlx::query_as(
        "SELECT id, scope FROM api_token WHERE token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(hash_token(token))
    .fetch_optional(&state.pool)
    .await?;
    let (token_id, scope) =
        row.ok_or_else(|| ApiError::unauthorized("invalid or revoked API token"))?;
    sqlx::query(
        "UPDATE api_token SET last_used_at = now() \
         WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')",
    )
    .bind(token_id)
    .execute(&state.pool)
    .await?;
    Ok(Authenticated { token_id, scope })
}

async fn require(
    state: AppState,
    required: TokenScope,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token =
        bearer(&request).ok_or_else(|| ApiError::unauthorized("missing bearer API token"))?;
    let auth = authenticate(&state, token).await?;
    if !auth.scope.allows(required) {
        return Err(ApiError::forbidden(format!(
            "this endpoint needs a token with scope {required:?}, the token has {:?}",
            auth.scope
        )));
    }
    request.extensions_mut().insert(auth);
    Ok(next.run(request).await)
}

pub async fn require_read(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    require(state, TokenScope::Read, request, next).await
}

pub async fn require_annotate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    require(state, TokenScope::Annotate, request, next).await
}

pub async fn require_write(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    require(state, TokenScope::Write, request, next).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_and_prefixed() {
        let a = generate_token();
        let b = generate_token();
        assert!(a.starts_with(TOKEN_PREFIX));
        assert_eq!(a.len(), TOKEN_PREFIX.len() + 43);
        assert_ne!(a, b);
    }

    #[test]
    fn hash_is_sha256() {
        assert_eq!(hash_token("abc").len(), 32);
        assert_eq!(
            hex::encode(hash_token("abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parses_bearer_header() {
        let request = |value: &str| {
            Request::builder()
                .header(header::AUTHORIZATION, value)
                .body(().into())
                .unwrap()
        };
        assert_eq!(bearer(&request("Bearer trk_x")), Some("trk_x"));
        assert_eq!(bearer(&request("bearer  trk_x ")), Some("trk_x"));
        assert_eq!(bearer(&request("Basic abc")), None);
        let none: Request = Request::builder().body(().into()).unwrap();
        assert_eq!(bearer(&none), None);
    }
}
