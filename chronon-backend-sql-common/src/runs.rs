//! Run persistence and worker claim helpers.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore); not a stable public API.

use chrono::{DateTime, Duration, Utc};

use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{Run, RunStatus};

use crate::backend::SqlDialect;
use crate::error_map::map_err;
use crate::row::{row_to_run, run_status_to_str, RunRow};
use crate::{bind_sql, sql_execute, sql_fetch_all_map, sql_fetch_optional_map, SqlSchedulerStore};

pub(crate) async fn create_run(store: &SqlSchedulerStore, run: &Run) -> Result<()> {
    let row = RunRow::from_model(run)?;
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_run (
            run_id, job_id, script_name, parent_run_id, root_run_id, child_index,
            scheduled_for, started_at, finished_at, duration_ms, status, attempt,
            instance_id, placement_json, pool_id, actor_json, params_json, stdout_text,
            stderr_text, error_json, stats_json, claimed_by, claim_lease_until
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
        )",
    );
    match sql_execute!(store, &sql, |q| {
        q.bind(&row.run_id)
            .bind(&row.job_id)
            .bind(&row.script_name)
            .bind(&row.parent_run_id)
            .bind(&row.root_run_id)
            .bind(row.child_index)
            .bind(row.scheduled_for)
            .bind(row.started_at)
            .bind(row.finished_at)
            .bind(row.duration_ms)
            .bind(&row.status)
            .bind(row.attempt)
            .bind(&row.instance_id)
            .bind(&row.placement_json)
            .bind(&row.pool_id)
            .bind(&row.actor_json)
            .bind(&row.params_json)
            .bind(&row.stdout_text)
            .bind(&row.stderr_text)
            .bind(&row.error_json)
            .bind(&row.stats_json)
            .bind(&row.claimed_by)
            .bind(row.claim_lease_until)
    }) {
        Ok(()) => Ok(()),
        Err(ChrononError::StorageError(msg))
            if msg.contains("UNIQUE") || msg.contains("unique") || msg.contains("duplicate") =>
        {
            Err(ChrononError::StorageError(format!(
                "duplicate run_id: {}",
                run.run_id
            )))
        }
        Err(e) => Err(e),
    }
}

pub(crate) async fn update_run(store: &SqlSchedulerStore, run: &Run) -> Result<()> {
    let row = RunRow::from_model(run)?;
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_run SET
            job_id = ?, script_name = ?, parent_run_id = ?, root_run_id = ?, child_index = ?,
            scheduled_for = ?, started_at = ?, finished_at = ?, duration_ms = ?, status = ?,
            attempt = ?, instance_id = ?, placement_json = ?, pool_id = ?, actor_json = ?,
            params_json = ?, stdout_text = ?, stderr_text = ?, error_json = ?, stats_json = ?,
            claimed_by = ?, claim_lease_until = ?
         WHERE run_id = ?",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(&row.job_id)
            .bind(&row.script_name)
            .bind(&row.parent_run_id)
            .bind(&row.root_run_id)
            .bind(row.child_index)
            .bind(row.scheduled_for)
            .bind(row.started_at)
            .bind(row.finished_at)
            .bind(row.duration_ms)
            .bind(&row.status)
            .bind(row.attempt)
            .bind(&row.instance_id)
            .bind(&row.placement_json)
            .bind(&row.pool_id)
            .bind(&row.actor_json)
            .bind(&row.params_json)
            .bind(&row.stdout_text)
            .bind(&row.stderr_text)
            .bind(&row.error_json)
            .bind(&row.stats_json)
            .bind(&row.claimed_by)
            .bind(row.claim_lease_until)
            .bind(&row.run_id)
    })
}

pub(crate) async fn get_run(store: &SqlSchedulerStore, run_id: &str) -> Result<Option<Run>> {
    let sql = bind_sql(store.dialect, "SELECT * FROM chronon_run WHERE run_id = ?");
    sql_fetch_optional_map!(store, &sql, |q| q.bind(run_id), |r| row_to_run(&r))
}

pub(crate) async fn list_runs_for_job(
    store: &SqlSchedulerStore,
    job_id: &str,
    limit: usize,
) -> Result<Vec<Run>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_run WHERE job_id = ? ORDER BY scheduled_for DESC LIMIT ?",
    );
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    sql_fetch_all_map!(store, &sql, |q| q.bind(job_id).bind(limit), |r| row_to_run(r))
}

pub(crate) async fn list_runs_filtered(
    store: &SqlSchedulerStore,
    job_id: Option<&str>,
    status: Option<RunStatus>,
    offset: usize,
    limit: usize,
) -> Result<Vec<Run>> {
    let offset = i64::try_from(offset).unwrap_or(i64::MAX);
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    match (job_id, status) {
        (Some(job_id), Some(status)) => {
            let sql = bind_sql(
                store.dialect,
                "SELECT * FROM chronon_run WHERE job_id = ? AND status = ?
                 ORDER BY scheduled_for DESC LIMIT ? OFFSET ?",
            );
            sql_fetch_all_map!(store, &sql, |q| {
                q.bind(job_id)
                    .bind(run_status_to_str(status))
                    .bind(limit)
                    .bind(offset)
            }, |r| row_to_run(r))
        }
        (Some(job_id), None) => {
            let sql = bind_sql(
                store.dialect,
                "SELECT * FROM chronon_run WHERE job_id = ?
                 ORDER BY scheduled_for DESC LIMIT ? OFFSET ?",
            );
            sql_fetch_all_map!(store, &sql, |q| q.bind(job_id).bind(limit).bind(offset), |r| {
                row_to_run(r)
            })
        }
        (None, Some(status)) => {
            let sql = bind_sql(
                store.dialect,
                "SELECT * FROM chronon_run WHERE status = ?
                 ORDER BY scheduled_for DESC LIMIT ? OFFSET ?",
            );
            sql_fetch_all_map!(store, &sql, |q| {
                q.bind(run_status_to_str(status)).bind(limit).bind(offset)
            }, |r| row_to_run(r))
        }
        (None, None) => {
            let sql = bind_sql(
                store.dialect,
                "SELECT * FROM chronon_run ORDER BY scheduled_for DESC LIMIT ? OFFSET ?",
            );
            sql_fetch_all_map!(store, &sql, |q| q.bind(limit).bind(offset), |r| row_to_run(r))
        }
    }
}

fn claim_next_queued_sql(dialect: SqlDialect) -> String {
    let skip = match dialect {
        SqlDialect::Postgres => " FOR UPDATE SKIP LOCKED",
        SqlDialect::Sqlite => "",
    };
    bind_sql(
        dialect,
        &format!(
            "UPDATE chronon_run SET
                status = 'claimed',
                claimed_by = ?,
                claim_lease_until = ?
             WHERE run_id = (
                SELECT run_id FROM chronon_run
                WHERE status = 'queued'
                  AND scheduled_for <= ?
                  AND (claim_lease_until IS NULL OR claim_lease_until <= ?)
                  AND (
                    pool_id = ?
                    OR (? = 'general' AND (pool_id IS NULL OR pool_id = 'general'))
                  )
                ORDER BY scheduled_for ASC
                LIMIT 1{skip}
             )
             RETURNING *"
        ),
    )
}

pub(crate) async fn claim_next_queued(
    store: &SqlSchedulerStore,
    pool_id: &str,
    worker_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<Option<Run>> {
    let lease_until = now + Duration::seconds(lease_ttl_secs);
    let sql = claim_next_queued_sql(store.dialect);
    sql_fetch_optional_map!(store, &sql, |q| {
        q.bind(worker_id)
            .bind(lease_until)
            .bind(now)
            .bind(now)
            .bind(pool_id)
            .bind(pool_id)
    }, |r| row_to_run(&r))
}

fn claim_run_by_id_sql(dialect: SqlDialect) -> String {
    bind_sql(
        dialect,
        "UPDATE chronon_run SET
            status = 'claimed',
            claimed_by = ?,
            claim_lease_until = ?
         WHERE run_id = ?
           AND status = 'queued'
           AND scheduled_for <= ?
           AND (claim_lease_until IS NULL OR claim_lease_until <= ?)
           AND (
             pool_id = ?
             OR (? = 'general' AND (pool_id IS NULL OR pool_id = 'general'))
           )
         RETURNING *",
    )
}

pub(crate) async fn claim_run_by_id(
    store: &SqlSchedulerStore,
    run_id: &str,
    pool_id: &str,
    worker_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<Option<Run>> {
    let lease_until = now + Duration::seconds(lease_ttl_secs);
    let sql = claim_run_by_id_sql(store.dialect);
    sql_fetch_optional_map!(store, &sql, |q| {
        q.bind(worker_id)
            .bind(lease_until)
            .bind(run_id)
            .bind(now)
            .bind(now)
            .bind(pool_id)
            .bind(pool_id)
    }, |r| row_to_run(&r))
}

fn claim_runs_by_ids_postgres_sql() -> &'static str {
    "UPDATE chronon_run SET
            status = 'claimed',
            claimed_by = $1,
            claim_lease_until = $2
         WHERE run_id = ANY($3)
           AND status = 'queued'
           AND scheduled_for <= $4
           AND (claim_lease_until IS NULL OR claim_lease_until <= $4)
           AND (
             pool_id = $5
             OR ($6 = 'general' AND (pool_id IS NULL OR pool_id = 'general'))
           )
         RETURNING *"
}

pub(crate) async fn claim_runs_by_ids(
    store: &SqlSchedulerStore,
    run_ids: &[&str],
    pool_id: &str,
    worker_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<Vec<Run>> {
    if run_ids.is_empty() {
        return Ok(Vec::new());
    }
    if store.dialect != SqlDialect::Postgres || run_ids.len() == 1 {
        let mut claimed = Vec::new();
        for run_id in run_ids {
            if let Some(run) =
                claim_run_by_id(store, run_id, pool_id, worker_id, now, lease_ttl_secs).await?
            {
                claimed.push(run);
            }
        }
        return Ok(claimed);
    }
    let lease_until = now + Duration::seconds(lease_ttl_secs);
    let ids: Vec<String> = run_ids.iter().map(|s| (*s).to_string()).collect();
    let sql = claim_runs_by_ids_postgres_sql();
    match &store.pool {
        crate::SqlPool::Postgres(pool) => {
            let rows = sqlx::query(sql)
                .bind(worker_id)
                .bind(lease_until)
                .bind(ids)
                .bind(now)
                .bind(pool_id)
                .bind(pool_id)
                .fetch_all(pool)
                .await
                .map_err(|e| map_err(&e))?;
            rows.iter().map(row_to_run).collect()
        }
        crate::SqlPool::Sqlite(_) => {
            let mut claimed = Vec::new();
            for run_id in run_ids {
                if let Some(run) =
                    claim_run_by_id(store, run_id, pool_id, worker_id, now, lease_ttl_secs).await?
                {
                    claimed.push(run);
                }
            }
            Ok(claimed)
        }
    }
}

pub(crate) async fn renew_run_lease(
    store: &SqlSchedulerStore,
    run_id: &str,
    worker_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<bool> {
    let lease_until = now + Duration::seconds(lease_ttl_secs);
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_run SET claim_lease_until = ?
         WHERE run_id = ?
           AND claimed_by = ?
           AND status IN ('claimed', 'running')",
    );
    let rows = match &store.pool {
        crate::SqlPool::Sqlite(pool) => {
            sqlx::query(&sql)
                .bind(lease_until)
                .bind(run_id)
                .bind(worker_id)
                .execute(pool)
                .await
                .map_err(|e| map_err(&e))?
                .rows_affected()
        }
        crate::SqlPool::Postgres(pool) => {
            sqlx::query(&sql)
                .bind(lease_until)
                .bind(run_id)
                .bind(worker_id)
                .execute(pool)
                .await
                .map_err(|e| map_err(&e))?
                .rows_affected()
        }
    };
    Ok(rows > 0)
}
