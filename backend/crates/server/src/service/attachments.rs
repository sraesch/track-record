//! Attachment upload (FR-5, ADR 0003): file first, then the metadata row.

use bytes::Bytes;
use futures::Stream;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::attachment_store::WriteError;
use crate::error::{ApiError, ApiResult};

/// Query parameters of an attachment upload. The body is the raw file;
/// `Content-Type` is stored and used for download.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct UploadAttachment {
    /// Original file name, used only for download (never as a path).
    pub file_name: String,
    /// Case execution of the run the file belongs to.
    pub case_execution_id: Option<Uuid>,
    /// Sample of that case execution the file belongs to.
    pub sample_id: Option<Uuid>,
}

pub struct UploadHeaders {
    pub content_type: String,
    /// `X-Content-SHA256`: expected SHA-256 of the body, hex.
    pub expected_sha256: Option<String>,
}

/// Stores an attachment. Same id and content again is a no-op (returns
/// false); same id with other content or metadata is a conflict.
pub async fn upload<S, E>(
    state: &AppState,
    run_id: Uuid,
    attachment_id: Uuid,
    params: &UploadAttachment,
    headers: &UploadHeaders,
    body: S,
) -> ApiResult<bool>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: std::fmt::Display,
{
    let file_name = params.file_name.trim();
    if file_name.is_empty() {
        return Err(ApiError::invalid("file_name must not be empty"));
    }
    if headers.content_type.trim().is_empty() {
        return Err(ApiError::invalid("Content-Type must not be empty"));
    }
    if params.sample_id.is_some() && params.case_execution_id.is_none() {
        return Err(ApiError::invalid("sample_id needs case_execution_id"));
    }
    let expected = headers
        .expected_sha256
        .as_deref()
        .map(|h| hex::decode(h.trim()).ok().filter(|b| b.len() == 32))
        .map(|b| b.ok_or_else(|| ApiError::bad_request("X-Content-SHA256 must be 64 hex digits")))
        .transpose()?;

    // Check the owners before writing, so a bad request leaves no file.
    let owner_ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM run WHERE id = $1) \
            AND ($2::uuid IS NULL OR EXISTS ( \
                SELECT 1 FROM case_execution WHERE id = $2 AND run_id = $1)) \
            AND ($3::uuid IS NULL OR EXISTS ( \
                SELECT 1 FROM sample WHERE id = $3 AND case_execution_id = $2))",
    )
    .bind(run_id)
    .bind(params.case_execution_id)
    .bind(params.sample_id)
    .fetch_one(&state.pool)
    .await?;
    if !owner_ok {
        return Err(ApiError::not_found(
            "run, case execution or sample not found (they must belong together)",
        ));
    }

    let temp = state
        .attachments
        .write_temp(run_id, body)
        .await
        .map_err(|e| match e {
            WriteError::Body(msg) => ApiError::bad_request(msg),
            WriteError::Io(e) => ApiError::Internal(e.into()),
        })?;
    if let Some(expected) = &expected
        && expected.as_slice() != temp.sha256
    {
        return Err(ApiError::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "checksum_mismatch",
            "the body does not match X-Content-SHA256",
        ));
    }

    let mut tx = state.pool.begin().await?;
    // Serializes uploads of the same attachment id across replicas.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(attachment_id)
        .execute(&mut *tx)
        .await?;
    let stored: Option<(
        Uuid,
        Option<Uuid>,
        Option<Uuid>,
        String,
        String,
        i64,
        Vec<u8>,
    )> = sqlx::query_as(
        "SELECT run_id, case_execution_id, sample_id, file_name, content_type, size_bytes, sha256 \
             FROM attachment WHERE id = $1",
    )
    .bind(attachment_id)
    .fetch_optional(&mut *tx)
    .await?;
    let size = i64::try_from(temp.size).map_err(|_| ApiError::invalid("file too large"))?;
    let wanted = (
        run_id,
        params.case_execution_id,
        params.sample_id,
        file_name.to_owned(),
        headers.content_type.clone(),
        size,
        temp.sha256.to_vec(),
    );
    if let Some(stored) = stored {
        return if stored == wanted {
            Ok(false)
        } else {
            Err(ApiError::conflict(format!(
                "attachment {attachment_id} already exists with different content"
            )))
        };
    }

    let sha256 = temp.sha256;
    state
        .attachments
        .commit(temp, run_id, attachment_id)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO attachment (id, run_id, case_execution_id, sample_id, file_name, content_type, \
                                 size_bytes, sha256) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(attachment_id)
    .bind(run_id)
    .bind(params.case_execution_id)
    .bind(params.sample_id)
    .bind(file_name)
    .bind(&headers.content_type)
    .bind(size)
    .bind(sha256.as_slice())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}
