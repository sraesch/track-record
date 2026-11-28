//! Checking a chart spec (ADR 0022, "Spec"): well-formed, valid, and the
//! output of each node, for the editor.

use sqlx::PgPool;
use track_record_core::chart::{self, Spec};

use super::env::ChartEnv;
use super::model::{ChartValidation, ValidateChart};
use crate::error::ApiResult;

/// Checks a spec. Problems are part of the answer, never an error: a spec
/// that is not even well-formed gives its problems.
pub async fn validate(pool: &PgPool, req: &ValidateChart) -> ApiResult<ChartValidation> {
    let spec = Spec::from_json(&req.spec).ok();
    let mut env = ChartEnv::load(&mut *pool.acquire().await?, spec.as_ref()).await?;
    let catalogue = env.catalogue.clone();
    Ok(chart::check(&req.spec, &mut env, &catalogue).into())
}
