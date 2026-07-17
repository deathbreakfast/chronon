//! Run-once and tick claim helpers.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore); not a stable public API.

use chrono::{DateTime, Duration, Utc};

use chronon_core::error::{ChrononError, Result};
use sqlx::Row;

use crate::error_map::map_err;
use crate::{bind_sql, sql_execute, sql_fetch_all_map, sql_fetch_one_map, SqlSchedulerStore};

pub(crate) async fn try_claim_run_once(
    store: &SqlSchedulerStore,
    job_id: &str,
    claimed_by: &str,
    now: DateTime<Utc>,
    claim_ttl_secs: i64,
) -> Result<bool> {
    let expires = now + Duration::seconds(claim_ttl_secs);
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_job SET
            run_once_claimed_at = ?,
            run_once_claimed_by = ?,
            run_once_claim_expires_at = ?,
            updated_at = ?
         WHERE job_id = ?
           AND run_once_completed_at IS NULL
           AND (
             run_once_claimed_at IS NULL
             OR run_once_claim_expires_at <= ?
             OR run_once_claimed_by = ?
           )",
    );
    let rows = match &store.pool {
        crate::SqlPool::Sqlite(pool) => {
            sqlx::query(&sql)
                .bind(now)
                .bind(claimed_by)
                .bind(expires)
                .bind(now)
                .bind(job_id)
                .bind(now)
                .bind(claimed_by)
                .execute(pool)
                .await
                .map_err(|e| map_err(&e))?
                .rows_affected()
        }
        crate::SqlPool::Postgres(pool) => {
            sqlx::query(&sql)
                .bind(now)
                .bind(claimed_by)
                .bind(expires)
                .bind(now)
                .bind(job_id)
                .bind(now)
                .bind(claimed_by)
                .execute(pool)
                .await
                .map_err(|e| map_err(&e))?
                .rows_affected()
        }
    };
    Ok(rows > 0)
}

pub(crate) async fn mark_run_once_completed(
    store: &SqlSchedulerStore,
    job_id: &str,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_job SET
            run_once_completed_at = ?,
            run_once_claimed_at = NULL,
            run_once_claimed_by = NULL,
            run_once_claim_expires_at = NULL,
            updated_at = ?
         WHERE job_id = ?",
    );
    sql_execute!(store, &sql, |q| q.bind(completed_at).bind(completed_at).bind(job_id))?;
    Ok(())
}

pub(crate) async fn release_run_once_claim(
    store: &SqlSchedulerStore,
    job_id: &str,
    claimed_by: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_job SET
            run_once_claimed_at = NULL,
            run_once_claimed_by = NULL,
            run_once_claim_expires_at = NULL,
            updated_at = ?
         WHERE job_id = ? AND run_once_claimed_by = ?",
    );
    sql_execute!(store, &sql, |q| q.bind(now).bind(job_id).bind(claimed_by))
}

pub(crate) async fn find_due_job_ids_in_partitions(
    store: &SqlSchedulerStore,
    owned_partitions: &[u32],
    due_until: DateTime<Utc>,
    limit: u32,
) -> Result<Vec<String>> {
    if owned_partitions.is_empty() || limit == 0 {
        return Ok(vec![]);
    }
    let placeholders = owned_partitions
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let now = Utc::now();
    let sql = bind_sql(
        store.dialect,
        &format!(
            "SELECT job_id FROM chronon_job
             WHERE enabled = 1
               AND schedule_kind != 'manual'
               AND next_run_at IS NOT NULL
               AND next_run_at <= ?
               AND partition_hash IN ({placeholders})
               AND (claim_lease_until IS NULL OR claim_lease_until < ?)
               AND (schedule_kind != 'run_once' OR run_once_completed_at IS NULL)
             ORDER BY next_run_at ASC
             LIMIT ?"
        ),
    );
    sql_fetch_all_map!(store, &sql, |q| {
        let mut q = q.bind(due_until);
        for p in owned_partitions {
            q = q.bind(i64::from(*p));
        }
        q.bind(now).bind(i64::from(limit))
    }, |r| {
        Ok::<String, ChrononError>(r.get("job_id"))
    })
}

pub(crate) async fn min_next_run_at_in_partitions(
    store: &SqlSchedulerStore,
    owned_partitions: &[u32],
) -> Result<Option<DateTime<Utc>>> {
    if owned_partitions.is_empty() {
        return Ok(None);
    }
    let placeholders = owned_partitions
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = bind_sql(
        store.dialect,
        &format!(
            "SELECT MIN(next_run_at) AS min_next FROM chronon_job
             WHERE enabled = 1
               AND schedule_kind != 'manual'
               AND next_run_at IS NOT NULL
               AND partition_hash IN ({placeholders})"
        ),
    );
    sql_fetch_one_map!(store, &sql, |q| {
        let mut q = q;
        for p in owned_partitions {
            q = q.bind(i64::from(*p));
        }
        q
    }, |r| {
        Ok(r.try_get::<Option<DateTime<Utc>>, _>("min_next")
            .map_err(|e| map_err(&e))?)
    })
}

pub(crate) async fn claim_job_for_tick(
    store: &SqlSchedulerStore,
    job_id: &str,
    claim_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<bool> {
    let until = now + Duration::seconds(lease_ttl_secs);
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_job SET
            claim_lease_id = ?,
            claim_lease_until = ?,
            updated_at = ?
         WHERE job_id = ?
           AND enabled = 1
           AND (claim_lease_until IS NULL OR claim_lease_until < ?)",
    );
    let rows = match &store.pool {
        crate::SqlPool::Sqlite(pool) => {
            sqlx::query(&sql)
                .bind(claim_id)
                .bind(until)
                .bind(now)
                .bind(job_id)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|e| map_err(&e))?
                .rows_affected()
        }
        crate::SqlPool::Postgres(pool) => {
            sqlx::query(&sql)
                .bind(claim_id)
                .bind(until)
                .bind(now)
                .bind(job_id)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|e| map_err(&e))?
                .rows_affected()
        }
    };
    Ok(rows > 0)
}

pub(crate) async fn release_job_tick_claim(
    store: &SqlSchedulerStore,
    job_id: &str,
) -> Result<()> {
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_job SET claim_lease_id = NULL, claim_lease_until = NULL, updated_at = ?
         WHERE job_id = ?",
    );
    let now = Utc::now();
    sql_execute!(store, &sql, |q| q.bind(now).bind(job_id))
}

pub(crate) async fn persist_post_tick_job_state(
    store: &SqlSchedulerStore,
    job_id: &str,
    next_run_at: Option<DateTime<Utc>>,
) -> Result<()> {
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_job SET
            next_run_at = ?,
            claim_lease_id = NULL,
            claim_lease_until = NULL,
            updated_at = ?
         WHERE job_id = ?",
    );
    let now = Utc::now();
    sql_execute!(store, &sql, |q| q.bind(next_run_at).bind(now).bind(job_id))
}
