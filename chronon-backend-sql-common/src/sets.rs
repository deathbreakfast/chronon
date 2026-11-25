//! Child sets, shard sets, run outputs and active-run counts for the SQL store.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore); not a stable public API.

use chrono::{DateTime, Utc};
use serde_json::Value;

use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{ChildOutcome, ChildSet, Run, ShardSet};

use crate::error_map::map_err;
use crate::row::{encode_json, encode_json_opt, row_to_child_set, row_to_shard_set, RunRow};
use crate::runs::{bind_run_insert, map_duplicate_run, INSERT_RUN_SQL};
use crate::{
    bind_sql, sql_execute, sql_execute_rows, sql_fetch_all_map, sql_fetch_one_map,
    sql_fetch_optional_map, SqlPool, SqlSchedulerStore,
};

/// Attempts at the read-then-compare-and-set checkpoint write before giving up.
const CHECKPOINT_CAS_ATTEMPTS: usize = 64;

const INSERT_CHILD_SET_SQL: &str = "INSERT INTO chronon_child_set (
    set_id, parent_run_id, child_job_id, count, succeeded, failed, skipped, settled_at,
    created_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) async fn set_run_outputs(
    store: &SqlSchedulerStore,
    run_id: &str,
    outputs: &Value,
) -> Result<()> {
    let encoded = encode_json(outputs)?;
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_run SET outputs_json = ? WHERE run_id = ?",
    );
    let rows = sql_execute_rows!(store, &sql, |q| q.bind(&encoded).bind(run_id))?;
    if rows == 0 {
        return Err(ChrononError::RunNotFound(run_id.to_string()));
    }
    Ok(())
}

pub(crate) async fn count_active_runs_for_job(
    store: &SqlSchedulerStore,
    job_id: &str,
) -> Result<i64> {
    let sql = bind_sql(
        store.dialect,
        "SELECT COUNT(*) AS active FROM chronon_run
         WHERE job_id = ? AND status IN ('claimed', 'running')",
    );
    sql_fetch_one_map!(store, &sql, |q| q.bind(job_id), |r| {
        use sqlx::Row;
        r.try_get::<i64, _>("active").map_err(map_err)
    })
}

/// Insert the set and its runs inside one transaction on `$pool`. Dropping the transaction on
/// the first error rolls everything back.
macro_rules! insert_child_set_tx {
    ($pool:expr, $dialect:expr, $set:expr, $rows:expr) => {{
        let mut tx = $pool.begin().await.map_err(map_err)?;
        sqlx::query(&bind_sql($dialect, INSERT_CHILD_SET_SQL))
            .bind(&$set.set_id)
            .bind(&$set.parent_run_id)
            .bind(&$set.child_job_id)
            .bind(i64::from($set.count))
            .bind(i64::from($set.succeeded))
            .bind(i64::from($set.failed))
            .bind(i64::from($set.skipped))
            .bind($set.settled_at)
            .bind($set.created_at)
            .execute(&mut *tx)
            .await
            .map_err(map_err)?;
        let insert_run = bind_sql($dialect, INSERT_RUN_SQL);
        for row in $rows {
            bind_run_insert!(sqlx::query(&insert_run), row)
                .execute(&mut *tx)
                .await
                .map_err(|e| map_duplicate_run(map_err(e), &row.run_id))?;
        }
        tx.commit().await.map_err(map_err)?;
    }};
}

pub(crate) async fn create_child_set_with_runs(
    store: &SqlSchedulerStore,
    set: &ChildSet,
    runs: &[Run],
) -> Result<()> {
    let rows = runs
        .iter()
        .map(RunRow::from_model)
        .collect::<Result<Vec<_>>>()?;
    match &store.pool {
        SqlPool::Sqlite(pool) => insert_child_set_tx!(pool, store.dialect, set, &rows),
        SqlPool::Postgres(pool) => insert_child_set_tx!(pool, store.dialect, set, &rows),
    }
    Ok(())
}

pub(crate) async fn get_child_set(
    store: &SqlSchedulerStore,
    set_id: &str,
) -> Result<Option<ChildSet>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_child_set WHERE set_id = ?",
    );
    sql_fetch_optional_map!(store, &sql, |q| q.bind(set_id), |r| row_to_child_set(&r))
}

pub(crate) async fn list_child_sets_for_run(
    store: &SqlSchedulerStore,
    parent_run_id: &str,
) -> Result<Vec<ChildSet>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_child_set WHERE parent_run_id = ?
         ORDER BY created_at ASC, set_id ASC",
    );
    sql_fetch_all_map!(store, &sql, |q| q.bind(parent_run_id), |r| {
        row_to_child_set(r)
    })
}

pub(crate) async fn record_child_result(
    store: &SqlSchedulerStore,
    set_id: &str,
    outcome: ChildOutcome,
) -> Result<ChildSet> {
    // One UPDATE ... RETURNING, so concurrent callers each see their own increment.
    let sql = bind_sql(
        store.dialect,
        match outcome {
            ChildOutcome::Succeeded => {
                "UPDATE chronon_child_set SET succeeded = succeeded + 1 WHERE set_id = ? RETURNING *"
            }
            ChildOutcome::Failed => {
                "UPDATE chronon_child_set SET failed = failed + 1 WHERE set_id = ? RETURNING *"
            }
            ChildOutcome::Skipped => {
                "UPDATE chronon_child_set SET skipped = skipped + 1 WHERE set_id = ? RETURNING *"
            }
        },
    );
    fetch_one_child_set(store, &sql, set_id)
        .await?
        .ok_or_else(|| ChrononError::storage(format!("child set not found: {set_id}")))
}

pub(crate) async fn settle_child_set(
    store: &SqlSchedulerStore,
    set_id: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_child_set SET settled_at = ? WHERE set_id = ? AND settled_at IS NULL",
    );
    sql_execute!(store, &sql, |q| q.bind(at).bind(set_id))
}

pub(crate) async fn create_shard_set(store: &SqlSchedulerStore, set: &ShardSet) -> Result<()> {
    let next_start = encode_json(&set.next_start)?;
    let end = encode_json_opt(set.end.as_ref())?;
    let checkpoints = encode_json(&set.checkpoints_json)?;
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_shard_set (
            set_id, parent_run_id, job_id, cursor_field, next_start, end_value,
            rows_per_shard, paused, checkpoints_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(&set.set_id)
            .bind(&set.parent_run_id)
            .bind(&set.job_id)
            .bind(&set.cursor_field)
            .bind(&next_start)
            .bind(&end)
            .bind(i64::from(set.rows_per_shard))
            .bind(i32::from(set.paused))
            .bind(&checkpoints)
            .bind(set.created_at)
    })
}

pub(crate) async fn get_shard_set(
    store: &SqlSchedulerStore,
    set_id: &str,
) -> Result<Option<ShardSet>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_shard_set WHERE set_id = ?",
    );
    sql_fetch_optional_map!(store, &sql, |q| q.bind(set_id), |r| row_to_shard_set(&r))
}

pub(crate) async fn update_shard_set(
    store: &SqlSchedulerStore,
    set: &ShardSet,
    expected_next_start: &Value,
) -> Result<bool> {
    let next_start = encode_json(&set.next_start)?;
    let end = encode_json_opt(set.end.as_ref())?;
    let expected = encode_json(expected_next_start)?;
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_shard_set
         SET next_start = ?, end_value = ?, rows_per_shard = ?, paused = ?
         WHERE set_id = ? AND next_start = ?",
    );
    let rows = sql_execute_rows!(store, &sql, |q| {
        q.bind(&next_start)
            .bind(&end)
            .bind(i64::from(set.rows_per_shard))
            .bind(i32::from(set.paused))
            .bind(&set.set_id)
            .bind(&expected)
    })?;
    Ok(rows > 0)
}

pub(crate) async fn set_shard_checkpoint(
    store: &SqlSchedulerStore,
    set_id: &str,
    shard_index: u32,
    value: &Value,
) -> Result<()> {
    let select = bind_sql(
        store.dialect,
        "SELECT checkpoints_json FROM chronon_shard_set WHERE set_id = ?",
    );
    let update = bind_sql(
        store.dialect,
        "UPDATE chronon_shard_set SET checkpoints_json = ?
         WHERE set_id = ? AND checkpoints_json = ?",
    );
    // Read, merge, then write only if nobody else wrote in between; retry when they did.
    for _ in 0..CHECKPOINT_CAS_ATTEMPTS {
        let current = read_checkpoints(store, &select, set_id).await?;
        let Some(current) = current else {
            return Err(ChrononError::storage(format!(
                "shard set not found: {set_id}"
            )));
        };
        let mut merged: Value = serde_json::from_str(&current)?;
        if !merged.is_object() {
            merged = Value::Object(serde_json::Map::default());
        }
        if let Value::Object(map) = &mut merged {
            map.insert(shard_index.to_string(), value.clone());
        }
        let encoded = encode_json(&merged)?;
        let rows = sql_execute_rows!(store, &update, |q| {
            q.bind(&encoded).bind(set_id).bind(&current)
        })?;
        if rows > 0 {
            return Ok(());
        }
    }
    Err(ChrononError::storage(format!(
        "shard set {set_id}: checkpoint write kept conflicting"
    )))
}

/// Run `sql` (one `set_id` bind) and map the optional child set row it returns.
async fn fetch_one_child_set(
    store: &SqlSchedulerStore,
    sql: &str,
    set_id: &str,
) -> Result<Option<ChildSet>> {
    sql_fetch_optional_map!(store, sql, |q| q.bind(set_id), |r| row_to_child_set(&r))
}

/// Raw `checkpoints_json` text of one shard set, as stored.
async fn read_checkpoints(
    store: &SqlSchedulerStore,
    sql: &str,
    set_id: &str,
) -> Result<Option<String>> {
    sql_fetch_optional_map!(store, sql, |q| q.bind(set_id), |r| {
        use sqlx::Row;
        r.try_get::<String, _>("checkpoints_json").map_err(map_err)
    })
}
