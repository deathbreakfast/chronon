//! Revisions, scripts, leader election, partitions, and workers.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore); not a stable public API.

use chrono::{DateTime, Duration, Utc};

use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{JobRevision, PartitionAssignment, SchedulerLeader, Script, Worker};
use sqlx::Row;

use crate::error_map::map_err;
use crate::row::{
    row_to_leader, row_to_partition, row_to_revision, row_to_script, JobRevisionRow,
    PartitionAssignmentRow, ScriptRow, WorkerRow,
};
use crate::{bind_sql, sql_execute, sql_fetch_all_map, sql_fetch_optional_map, SqlSchedulerStore};

/// Fixed primary key for the singleton leader election row.
pub const LEADER_ROW_ID: &str = "singleton";

pub(crate) async fn append_revision(
    store: &SqlSchedulerStore,
    revision: &JobRevision,
) -> Result<()> {
    let row = JobRevisionRow::from_model(revision)?;
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_job_revision (
            revision_id, job_id, revision_number, changed_at, changed_by_actor_json, snapshot_json
        ) VALUES (?, ?, ?, ?, ?, ?)",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(&row.revision_id)
            .bind(&row.job_id)
            .bind(row.revision_number)
            .bind(row.changed_at)
            .bind(&row.changed_by_actor_json)
            .bind(&row.snapshot_json)
    })
}

pub(crate) async fn list_revisions(
    store: &SqlSchedulerStore,
    job_id: &str,
) -> Result<Vec<JobRevision>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_job_revision WHERE job_id = ? ORDER BY revision_number ASC",
    );
    sql_fetch_all_map!(store, &sql, |q| q.bind(job_id), |r| row_to_revision(r))
}

pub(crate) async fn upsert_script(store: &SqlSchedulerStore, script: &Script) -> Result<()> {
    let row = ScriptRow::from_model(script)?;
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_script (script_id, script_name, signature_json, signature_hash, created_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (script_name) DO UPDATE SET
            script_id = excluded.script_id,
            signature_json = excluded.signature_json,
            signature_hash = excluded.signature_hash,
            created_at = excluded.created_at",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(&row.script_id)
            .bind(&row.script_name)
            .bind(&row.signature_json)
            .bind(&row.signature_hash)
            .bind(row.created_at)
    })
}

pub(crate) async fn get_script(
    store: &SqlSchedulerStore,
    script_name: &str,
) -> Result<Option<Script>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_script WHERE script_name = ?",
    );
    sql_fetch_optional_map!(store, &sql, |q| q.bind(script_name), |r| row_to_script(&r))
}

pub(crate) async fn try_acquire_leader(
    store: &SqlSchedulerStore,
    instance_id: &str,
    ttl_secs: i64,
) -> Result<bool> {
    let now = Utc::now();
    let until = now + Duration::seconds(ttl_secs);
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_scheduler_leader (
            leader_id, leader_instance_id, leader_lease_until, last_heartbeat_at
        ) VALUES (?, ?, ?, ?)
        ON CONFLICT (leader_id) DO UPDATE SET
            leader_instance_id = excluded.leader_instance_id,
            leader_lease_until = excluded.leader_lease_until,
            last_heartbeat_at = excluded.last_heartbeat_at
        WHERE chronon_scheduler_leader.leader_lease_until <= excluded.last_heartbeat_at
           OR chronon_scheduler_leader.leader_instance_id = excluded.leader_instance_id
        RETURNING leader_instance_id",
    );
    let acquired: Option<String> = match &store.pool {
        crate::SqlPool::Sqlite(pool) => {
            let q = sqlx::query(&sql)
                .bind(LEADER_ROW_ID)
                .bind(instance_id)
                .bind(until)
                .bind(now);
            match q.fetch_optional(pool).await.map_err(map_err)? {
                Some(row) => Some(row.try_get("leader_instance_id").map_err(map_err)?),
                None => None,
            }
        }
        crate::SqlPool::Postgres(pool) => {
            let q = sqlx::query(&sql)
                .bind(LEADER_ROW_ID)
                .bind(instance_id)
                .bind(until)
                .bind(now);
            match q.fetch_optional(pool).await.map_err(map_err)? {
                Some(row) => Some(row.try_get("leader_instance_id").map_err(map_err)?),
                None => None,
            }
        }
    };
    Ok(acquired.as_deref() == Some(instance_id))
}

pub(crate) async fn renew_leader_lease(
    store: &SqlSchedulerStore,
    instance_id: &str,
    ttl_secs: i64,
) -> Result<()> {
    let now = Utc::now();
    let until = now + Duration::seconds(ttl_secs);
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_scheduler_leader SET leader_lease_until = ?, last_heartbeat_at = ?
         WHERE leader_id = ? AND leader_instance_id = ?",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(until)
            .bind(now)
            .bind(LEADER_ROW_ID)
            .bind(instance_id)
    })
}

pub(crate) async fn get_leader(store: &SqlSchedulerStore) -> Result<Option<SchedulerLeader>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_scheduler_leader WHERE leader_id = ?",
    );
    sql_fetch_optional_map!(store, &sql, |q| q.bind(LEADER_ROW_ID), |r| row_to_leader(
        &r
    ))
}

pub(crate) async fn upsert_partition_assignment(
    store: &SqlSchedulerStore,
    assignment: &PartitionAssignment,
) -> Result<()> {
    let row = PartitionAssignmentRow::from_model(assignment);
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_partition_assignment (
            partition_id, owner_instance_id, lease_until, updated_at
        ) VALUES (?, ?, ?, ?)
        ON CONFLICT (partition_id) DO UPDATE SET
            owner_instance_id = excluded.owner_instance_id,
            lease_until = excluded.lease_until,
            updated_at = excluded.updated_at",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(&row.partition_id)
            .bind(&row.owner_instance_id)
            .bind(row.lease_until)
            .bind(row.updated_at)
    })
}

pub(crate) async fn list_partition_assignments(
    store: &SqlSchedulerStore,
) -> Result<Vec<PartitionAssignment>> {
    let sql = bind_sql(
        store.dialect,
        "SELECT * FROM chronon_partition_assignment ORDER BY partition_id ASC",
    );
    sql_fetch_all_map!(store, &sql, |q| q, |r| row_to_partition(r))
}

pub(crate) async fn register_worker(store: &SqlSchedulerStore, worker: &Worker) -> Result<()> {
    let row = WorkerRow::from_model(worker)?;
    let sql = bind_sql(
        store.dialect,
        "INSERT INTO chronon_worker (
            worker_id, pool_id, cell_id, status, last_heartbeat_at, capacity_json, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (worker_id) DO UPDATE SET
            pool_id = excluded.pool_id,
            cell_id = excluded.cell_id,
            status = excluded.status,
            last_heartbeat_at = excluded.last_heartbeat_at,
            capacity_json = excluded.capacity_json,
            updated_at = excluded.updated_at",
    );
    sql_execute!(store, &sql, |q| {
        q.bind(&row.worker_id)
            .bind(&row.pool_id)
            .bind(&row.cell_id)
            .bind(&row.status)
            .bind(row.last_heartbeat_at)
            .bind(&row.capacity_json)
            .bind(row.created_at)
            .bind(row.updated_at)
    })
}

pub(crate) async fn heartbeat_worker(
    store: &SqlSchedulerStore,
    worker_id: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    let sql = bind_sql(
        store.dialect,
        "UPDATE chronon_worker SET last_heartbeat_at = ?, updated_at = ? WHERE worker_id = ?",
    );
    let rows = match &store.pool {
        crate::SqlPool::Sqlite(pool) => sqlx::query(&sql)
            .bind(at)
            .bind(at)
            .bind(worker_id)
            .execute(pool)
            .await
            .map_err(map_err)?
            .rows_affected(),
        crate::SqlPool::Postgres(pool) => sqlx::query(&sql)
            .bind(at)
            .bind(at)
            .bind(worker_id)
            .execute(pool)
            .await
            .map_err(map_err)?
            .rows_affected(),
    };
    if rows == 0 {
        return Err(ChrononError::Internal(format!(
            "worker not registered: {worker_id}"
        )));
    }
    Ok(())
}
