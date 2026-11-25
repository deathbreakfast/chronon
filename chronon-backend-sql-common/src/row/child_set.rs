//! [`ChildSet`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::ChildSet;
use sqlx::{ColumnIndex, Row};

use super::decode_u32;
use crate::error_map::map_err;

/// Map a SQL row to a [`ChildSet`].
pub fn row_to_child_set<'r, R>(row: &'r R) -> Result<ChildSet>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<DateTime<Utc>>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let counter = |column: &str| -> Result<u32> {
        let value: i64 = row.try_get(column).map_err(map_err)?;
        decode_u32(column, value)
    };
    Ok(ChildSet {
        set_id: row.try_get("set_id").map_err(map_err)?,
        parent_run_id: row.try_get("parent_run_id").map_err(map_err)?,
        child_job_id: row.try_get("child_job_id").map_err(map_err)?,
        count: counter("count")?,
        succeeded: counter("succeeded")?,
        failed: counter("failed")?,
        skipped: counter("skipped")?,
        settled_at: row.try_get("settled_at").map_err(map_err)?,
        created_at: row.try_get("created_at").map_err(map_err)?,
    })
}
