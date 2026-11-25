//! [`ShardSet`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::ShardSet;
use sqlx::{ColumnIndex, Row};

use super::{decode_json, decode_json_opt, decode_u32};
use crate::error_map::map_err;

/// Map a SQL row to a [`ShardSet`].
pub fn row_to_shard_set<'r, R>(row: &'r R) -> Result<ShardSet>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i32: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    bool: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<String>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let paused: bool = if let Ok(b) = row.try_get::<bool, _>("paused") {
        b
    } else {
        let v: i32 = row.try_get("paused").map_err(map_err)?;
        v != 0
    };
    let next_start: String = row.try_get("next_start").map_err(map_err)?;
    let checkpoints: String = row.try_get("checkpoints_json").map_err(map_err)?;
    let rows_per_shard: i64 = row.try_get("rows_per_shard").map_err(map_err)?;
    Ok(ShardSet {
        set_id: row.try_get("set_id").map_err(map_err)?,
        parent_run_id: row.try_get("parent_run_id").map_err(map_err)?,
        job_id: row.try_get("job_id").map_err(map_err)?,
        cursor_field: row.try_get("cursor_field").map_err(map_err)?,
        next_start: decode_json(&next_start)?,
        end: decode_json_opt(row.try_get("end_value").map_err(map_err)?)?,
        rows_per_shard: decode_u32("rows_per_shard", rows_per_shard)?,
        paused,
        checkpoints_json: decode_json(&checkpoints)?,
        created_at: row.try_get("created_at").map_err(map_err)?,
    })
}
