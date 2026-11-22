//! Series store (ADR 0002, NFR-4).
//!
//! The ONLY module that reads or writes `series_point`. Everything else gets
//! points through these functions, so storage can move to arrays or
//! compressed blocks by changing this module alone. A test in
//! tests/invariants.rs enforces this.
//!
//! Points are stored as `real` (f32), one row per point. A series and all its
//! points are written in one transaction; the database checks completeness
//! and non-decreasing x at commit (TR005, TR006).

use std::collections::HashMap;

use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

/// Points of one series, in position order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Points {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
}

/// A series to be written.
#[derive(Debug, Clone)]
pub struct NewSeries<'a> {
    pub id: Uuid,
    pub sample_id: Uuid,
    pub metric_id: i64,
    pub x_dimension: &'a str,
    pub x_unit: &'a str,
    pub points: &'a Points,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// The series was created.
    Created,
    /// A series with this id and identical content already existed (retry).
    Unchanged,
    /// A series with this id exists with different content.
    Differs,
}

#[derive(sqlx::FromRow)]
struct StoredMeta {
    key: i64,
    sample_id: Uuid,
    metric_id: i64,
    x_dimension: String,
    x_unit: String,
    point_count: i32,
}

/// Writes a series and its points inside the caller's transaction, or
/// compares with the stored series of the same id (FR-3).
pub async fn insert(
    conn: &mut PgConnection,
    series: &NewSeries<'_>,
) -> sqlx::Result<InsertOutcome> {
    let count = i32::try_from(series.points.x.len()).expect("point count fits i32");
    let key: Option<i64> = sqlx::query_scalar(
        "INSERT INTO series (id, sample_id, metric_id, x_dimension, x_unit, point_count) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING RETURNING key",
    )
    .bind(series.id)
    .bind(series.sample_id)
    .bind(series.metric_id)
    .bind(series.x_dimension)
    .bind(series.x_unit)
    .bind(count)
    .fetch_optional(&mut *conn)
    .await?;

    if let Some(key) = key {
        sqlx::query(
            "INSERT INTO series_point (series_key, position, x, y) \
             SELECT $1, (p.ord - 1)::integer, p.x, p.y \
             FROM unnest($2::real[], $3::real[]) WITH ORDINALITY AS p(x, y, ord)",
        )
        .bind(key)
        .bind(&series.points.x)
        .bind(&series.points.y)
        .execute(&mut *conn)
        .await?;
        return Ok(InsertOutcome::Created);
    }

    let stored: StoredMeta = sqlx::query_as(
        "SELECT key, sample_id, metric_id, x_dimension, x_unit, point_count \
         FROM series WHERE id = $1",
    )
    .bind(series.id)
    .fetch_one(&mut *conn)
    .await?;
    let same_meta = stored.sample_id == series.sample_id
        && stored.metric_id == series.metric_id
        && stored.x_dimension == series.x_dimension
        && stored.x_unit == series.x_unit
        && stored.point_count == count;
    if !same_meta {
        return Ok(InsertOutcome::Differs);
    }
    let points = load_by_key(&mut *conn, stored.key).await?;
    Ok(if same_points(&points, series.points) {
        InsertOutcome::Unchanged
    } else {
        InsertOutcome::Differs
    })
}

/// Bitwise comparison, so -0.0 and 0.0 differ like in the stored values.
fn same_points(a: &Points, b: &Points) -> bool {
    let eq = |p: &[f32], q: &[f32]| {
        p.len() == q.len() && p.iter().zip(q).all(|(a, b)| a.to_bits() == b.to_bits())
    };
    eq(&a.x, &b.x) && eq(&a.y, &b.y)
}

/// Points of one series by its internal key.
pub async fn load_by_key(executor: impl PgExecutor<'_>, key: i64) -> sqlx::Result<Points> {
    let row: Option<(Vec<f32>, Vec<f32>)> = sqlx::query_as(
        "SELECT array_agg(x ORDER BY position), array_agg(y ORDER BY position) \
         FROM series_point WHERE series_key = $1 HAVING count(*) > 0",
    )
    .bind(key)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(x, y)| Points { x, y }).unwrap_or_default())
}

/// Points of one series by its client id, or `None` if it does not exist.
pub async fn load_by_id(executor: impl PgExecutor<'_>, id: Uuid) -> sqlx::Result<Option<Points>> {
    let row: Option<(Option<Vec<f32>>, Option<Vec<f32>>)> = sqlx::query_as(
        "SELECT (SELECT array_agg(x ORDER BY position) FROM series_point WHERE series_key = s.key), \
                (SELECT array_agg(y ORDER BY position) FROM series_point WHERE series_key = s.key) \
         FROM series s WHERE s.id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(x, y)| Points {
        x: x.unwrap_or_default(),
        y: y.unwrap_or_default(),
    }))
}

/// Points of many series by client id. Missing series are left out.
pub async fn load_many(
    executor: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<HashMap<Uuid, Points>> {
    let rows: Vec<(Uuid, Option<Vec<f32>>, Option<Vec<f32>>)> = sqlx::query_as(
        "SELECT s.id, \
                (SELECT array_agg(x ORDER BY position) FROM series_point WHERE series_key = s.key), \
                (SELECT array_agg(y ORDER BY position) FROM series_point WHERE series_key = s.key) \
         FROM series s WHERE s.id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, x, y)| {
            (
                id,
                Points {
                    x: x.unwrap_or_default(),
                    y: y.unwrap_or_default(),
                },
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_comparison_is_bitwise() {
        let a = Points {
            x: vec![0.0, 1.0],
            y: vec![0.0, 2.0],
        };
        let mut b = a.clone();
        assert!(same_points(&a, &b));
        b.y[0] = -0.0;
        assert!(!same_points(&a, &b));
        b.y.pop();
        assert!(!same_points(&a, &b));
    }
}
