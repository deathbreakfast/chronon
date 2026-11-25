//! Bootstrap SQL tables and indexes for the Chronon scheduler store.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore) on connect.

use chronon_core::Result;

use crate::{SqlDialect, SqlPool, SqlSchedulerStore};

const JOB_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_job (
    job_id TEXT PRIMARY KEY,
    job_name TEXT NOT NULL UNIQUE,
    script_name TEXT NOT NULL,
    script_sig_hash TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    schedule_kind TEXT NOT NULL,
    cron_expr TEXT,
    timezone TEXT,
    run_once_at TIMESTAMPTZ,
    run_once_claimed_at TIMESTAMPTZ,
    run_once_claimed_by TEXT,
    run_once_completed_at TIMESTAMPTZ,
    run_once_claim_expires_at TIMESTAMPTZ,
    partition_hash BIGINT,
    claim_lease_id TEXT,
    claim_lease_until TIMESTAMPTZ,
    pool TEXT,
    region TEXT,
    placement_json TEXT,
    actor_json TEXT NOT NULL,
    params_json TEXT NOT NULL,
    concurrency INTEGER NOT NULL,
    timeout_ms BIGINT,
    retry_policy_json TEXT NOT NULL,
    misfire_policy_json TEXT NOT NULL,
    parent_limits_json TEXT,
    parent_job_id TEXT,
    on_busy TEXT NOT NULL DEFAULT 'skip',
    sharding_json TEXT,
    next_run_at TIMESTAMPTZ,
    current_revision INTEGER NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
)";

const RUN_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_run (
    run_id TEXT PRIMARY KEY,
    job_id TEXT,
    script_name TEXT NOT NULL,
    parent_run_id TEXT,
    root_run_id TEXT,
    child_index INTEGER,
    child_set_id TEXT,
    scheduled_for TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration_ms BIGINT,
    status TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    instance_id TEXT,
    placement_json TEXT,
    pool_id TEXT,
    actor_json TEXT NOT NULL,
    params_json TEXT NOT NULL,
    run_values_json TEXT NOT NULL DEFAULT '{}',
    outputs_json TEXT NOT NULL DEFAULT '{}',
    stdout_text TEXT,
    stderr_text TEXT,
    error_json TEXT,
    stats_json TEXT,
    claimed_by TEXT,
    claim_lease_until TIMESTAMPTZ
)";

const REVISION_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_job_revision (
    revision_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL,
    revision_number INTEGER NOT NULL,
    changed_at TIMESTAMPTZ NOT NULL,
    changed_by_actor_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL
)";

const SCRIPT_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_script (
    script_id TEXT PRIMARY KEY,
    script_name TEXT NOT NULL UNIQUE,
    signature_json TEXT NOT NULL,
    signature_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
)";

const LEADER_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_scheduler_leader (
    leader_id TEXT PRIMARY KEY,
    leader_instance_id TEXT NOT NULL,
    leader_lease_until TIMESTAMPTZ NOT NULL,
    last_heartbeat_at TIMESTAMPTZ NOT NULL
)";

const PARTITION_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_partition_assignment (
    partition_id TEXT PRIMARY KEY,
    owner_instance_id TEXT NOT NULL,
    lease_until TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
)";

const WORKER_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_worker (
    worker_id TEXT PRIMARY KEY,
    pool_id TEXT NOT NULL,
    cell_id TEXT,
    status TEXT NOT NULL,
    last_heartbeat_at TIMESTAMPTZ NOT NULL,
    capacity_json TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
)";

const CHILD_SET_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_child_set (
    set_id TEXT PRIMARY KEY,
    parent_run_id TEXT NOT NULL,
    child_job_id TEXT NOT NULL,
    count BIGINT NOT NULL,
    succeeded BIGINT NOT NULL DEFAULT 0,
    failed BIGINT NOT NULL DEFAULT 0,
    skipped BIGINT NOT NULL DEFAULT 0,
    settled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
)";

const SHARD_SET_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_shard_set (
    set_id TEXT PRIMARY KEY,
    parent_run_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    cursor_field TEXT NOT NULL,
    next_start TEXT NOT NULL,
    end_value TEXT,
    rows_per_shard BIGINT NOT NULL,
    paused INTEGER NOT NULL DEFAULT 0,
    checkpoints_json TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL
)";

/// Columns added after the first release, as `(table, column, type and default)`.
///
/// Tables created by an older build get them through `ALTER TABLE ... ADD COLUMN`; every one
/// is nullable or has a default, so existing rows keep loading.
const ADDED_COLUMNS: [(&str, &str, &str); 6] = [
    ("chronon_job", "parent_job_id", "TEXT"),
    ("chronon_job", "on_busy", "TEXT NOT NULL DEFAULT 'skip'"),
    ("chronon_job", "sharding_json", "TEXT"),
    ("chronon_run", "child_set_id", "TEXT"),
    (
        "chronon_run",
        "run_values_json",
        "TEXT NOT NULL DEFAULT '{}'",
    ),
    ("chronon_run", "outputs_json", "TEXT NOT NULL DEFAULT '{}'"),
];

/// Create tables and indexes if missing.
pub async fn ensure_schema(store: &SqlSchedulerStore) -> Result<()> {
    if store.dialect() == SqlDialect::Postgres {
        let SqlPool::Postgres(pool) = store.pool() else {
            return Err(chronon_core::ChrononError::Internal(
                "postgres dialect without postgres pool".into(),
            ));
        };
        let mut conn = pool.acquire().await.map_err(crate::error_map::map_err)?;
        sqlx::query("SELECT pg_advisory_lock(872349013)")
            .execute(&mut *conn)
            .await
            .map_err(crate::error_map::map_err)?;
        let result = ensure_schema_tables_on_conn(&mut conn).await;
        let _ = sqlx::query("SELECT pg_advisory_unlock(872349013)")
            .execute(&mut *conn)
            .await;
        return result;
    }
    ensure_schema_tables(store).await
}

async fn ensure_schema_tables_on_conn(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
) -> Result<()> {
    for ddl in schema_table_ddls() {
        sqlx::query(ddl)
            .execute(&mut **conn)
            .await
            .map_err(crate::error_map::map_err)?;
    }
    for (table, column, ty) in ADDED_COLUMNS {
        sqlx::query(&format!(
            "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {column} {ty}"
        ))
        .execute(&mut **conn)
        .await
        .map_err(crate::error_map::map_err)?;
    }
    for ddl in schema_index_ddls() {
        sqlx::query(ddl)
            .execute(&mut **conn)
            .await
            .map_err(crate::error_map::map_err)?;
    }
    Ok(())
}

async fn ensure_schema_tables(store: &SqlSchedulerStore) -> Result<()> {
    for ddl in schema_table_ddls() {
        store.run_ddl(ddl).await?;
    }
    add_missing_sqlite_columns(store).await?;
    for ddl in schema_index_ddls() {
        store.run_ddl(ddl).await?;
    }
    Ok(())
}

/// SQLite has no `ADD COLUMN IF NOT EXISTS`, so check `pragma_table_info` first.
async fn add_missing_sqlite_columns(store: &SqlSchedulerStore) -> Result<()> {
    let SqlPool::Sqlite(pool) = store.pool() else {
        return Err(chronon_core::ChrononError::Internal(
            "sqlite column migration without sqlite pool".into(),
        ));
    };
    for (table, column, ty) in ADDED_COLUMNS {
        let present: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?")
                .bind(table)
                .bind(column)
                .fetch_one(pool)
                .await
                .map_err(crate::error_map::map_err)?;
        if present == 0 {
            store
                .run_ddl(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty}"))
                .await?;
        }
    }
    Ok(())
}

fn schema_table_ddls() -> [&'static str; 9] {
    [
        JOB_TABLE,
        RUN_TABLE,
        REVISION_TABLE,
        SCRIPT_TABLE,
        LEADER_TABLE,
        PARTITION_TABLE,
        WORKER_TABLE,
        CHILD_SET_TABLE,
        SHARD_SET_TABLE,
    ]
}

fn schema_index_ddls() -> [&'static str; 10] {
    [
        "CREATE INDEX IF NOT EXISTS chronon_job_name ON chronon_job (job_name)",
        "CREATE INDEX IF NOT EXISTS chronon_job_due_partitions ON chronon_job (next_run_at, partition_hash)
             WHERE enabled = 1 AND schedule_kind != 'manual' AND next_run_at IS NOT NULL",
        "CREATE INDEX IF NOT EXISTS chronon_run_job_id ON chronon_run (job_id)",
        "CREATE INDEX IF NOT EXISTS chronon_run_queued_pool ON chronon_run (pool_id, scheduled_for)
             WHERE status = 'queued'",
        "CREATE INDEX IF NOT EXISTS chronon_run_scheduled_for_run_id ON chronon_run (scheduled_for, run_id)",
        "CREATE INDEX IF NOT EXISTS chronon_job_revision_job ON chronon_job_revision (job_id, revision_number)",
        "CREATE INDEX IF NOT EXISTS chronon_run_job_status ON chronon_run (job_id, status)",
        "CREATE INDEX IF NOT EXISTS chronon_run_child_set ON chronon_run (child_set_id)",
        "CREATE INDEX IF NOT EXISTS chronon_child_set_parent ON chronon_child_set (parent_run_id)",
        "CREATE INDEX IF NOT EXISTS chronon_shard_set_parent ON chronon_shard_set (parent_run_id)",
    ]
}

#[cfg(test)]
mod tests {
    use crate::SqlSchedulerStore;

    #[tokio::test]
    async fn schema_idempotent_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let b1 = SqlSchedulerStore::connect_sqlite(&url).await.unwrap();
        let b2 = SqlSchedulerStore::connect_sqlite(&url).await.unwrap();
        drop(b1);
        drop(b2);
    }

    #[tokio::test]
    async fn sqlite_wal_concurrent_upserts_do_not_busy() {
        use std::sync::Arc;

        use chronon_core::models::Job;
        use chronon_core::store::SchedulerStore;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wal.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let store = Arc::new(SqlSchedulerStore::connect_sqlite(&url).await.unwrap());
        let mut joins = Vec::new();
        for task in 0..8 {
            let store = Arc::clone(&store);
            joins.push(tokio::spawn(async move {
                for i in 0..40 {
                    let job = Job::new(format!("w{task}-{i}"), "script_a");
                    store.upsert_job(&job).await.expect("upsert");
                }
            }));
        }
        for join in joins {
            join.await.expect("task");
        }
    }

    #[tokio::test]
    async fn sqlite_memory_url_still_connects() {
        SqlSchedulerStore::connect_sqlite("sqlite://:memory:")
            .await
            .expect("memory sqlite");
    }

    /// Tables as the first release created them, before child sets and run values.
    const PRE_CHILD_SET_JOB_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_job (
    job_id TEXT PRIMARY KEY,
    job_name TEXT NOT NULL UNIQUE,
    script_name TEXT NOT NULL,
    script_sig_hash TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    schedule_kind TEXT NOT NULL,
    cron_expr TEXT,
    timezone TEXT,
    run_once_at TIMESTAMPTZ,
    run_once_claimed_at TIMESTAMPTZ,
    run_once_claimed_by TEXT,
    run_once_completed_at TIMESTAMPTZ,
    run_once_claim_expires_at TIMESTAMPTZ,
    partition_hash BIGINT,
    claim_lease_id TEXT,
    claim_lease_until TIMESTAMPTZ,
    pool TEXT,
    region TEXT,
    placement_json TEXT,
    actor_json TEXT NOT NULL,
    params_json TEXT NOT NULL,
    concurrency INTEGER NOT NULL,
    timeout_ms BIGINT,
    retry_policy_json TEXT NOT NULL,
    misfire_policy_json TEXT NOT NULL,
    parent_limits_json TEXT,
    next_run_at TIMESTAMPTZ,
    current_revision INTEGER NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
)";

    const PRE_CHILD_SET_RUN_TABLE: &str = r"
CREATE TABLE IF NOT EXISTS chronon_run (
    run_id TEXT PRIMARY KEY,
    job_id TEXT,
    script_name TEXT NOT NULL,
    parent_run_id TEXT,
    root_run_id TEXT,
    child_index INTEGER,
    scheduled_for TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration_ms BIGINT,
    status TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    instance_id TEXT,
    placement_json TEXT,
    pool_id TEXT,
    actor_json TEXT NOT NULL,
    params_json TEXT NOT NULL,
    stdout_text TEXT,
    stderr_text TEXT,
    error_json TEXT,
    stats_json TEXT,
    claimed_by TEXT,
    claim_lease_until TIMESTAMPTZ
)";

    #[tokio::test]
    async fn rows_written_before_migration_load_with_defaults() {
        use chronon_core::models::{OnBusy, ScheduleKind};
        use chronon_core::store::SchedulerStore;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        sqlx::query(PRE_CHILD_SET_JOB_TABLE)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(PRE_CHILD_SET_RUN_TABLE)
            .execute(&pool)
            .await
            .unwrap();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO chronon_job (
                job_id, job_name, script_name, script_sig_hash, enabled, schedule_kind,
                actor_json, params_json, concurrency, retry_policy_json, misfire_policy_json,
                current_revision, updated_at, created_at
            ) VALUES ('j1', 'old-job', 's', '', 1, 'manual', 'null', '{}', 1, '{}', '{}', 1, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chronon_run (
                run_id, job_id, script_name, scheduled_for, status, attempt, actor_json,
                params_json
            ) VALUES ('r1', 'j1', 's', ?, 'queued', 1, 'null', '{}')",
        )
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqlSchedulerStore::connect_sqlite(&url).await.unwrap();
        let job = store.get_job("j1").await.unwrap().expect("job");
        assert_eq!(job.on_busy, OnBusy::Skip);
        assert_eq!(job.schedule_kind, ScheduleKind::Manual);
        assert!(job.sharding_json.is_none());
        let run = store.get_run("r1").await.unwrap().expect("run");
        assert_eq!(run.run_values_json, serde_json::json!({}));
        assert_eq!(run.outputs_json, serde_json::json!({}));
        assert!(run.child_set_id.is_none());
        drop(store);
        // A second connect finds the columns already there.
        SqlSchedulerStore::connect_sqlite(&url).await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_record_child_result_never_loses_a_count() {
        use std::sync::Arc;

        use chronon_core::models::{ChildOutcome, ChildSet};
        use chronon_core::store::SchedulerStore;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("counts.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let store = Arc::new(SqlSchedulerStore::connect_sqlite(&url).await.unwrap());
        let set = ChildSet::new("parent", "child-job", 200);
        store.create_child_set_with_runs(&set, &[]).await.unwrap();
        let mut joins = Vec::new();
        for task in 0..8 {
            let store = Arc::clone(&store);
            let set_id = set.set_id.clone();
            joins.push(tokio::spawn(async move {
                for i in 0..25 {
                    let outcome = match (task + i) % 3 {
                        0 => ChildOutcome::Succeeded,
                        1 => ChildOutcome::Failed,
                        _ => ChildOutcome::Skipped,
                    };
                    store.record_child_result(&set_id, outcome).await.unwrap();
                }
            }));
        }
        for join in joins {
            join.await.unwrap();
        }
        let stored = store.get_child_set(&set.set_id).await.unwrap().unwrap();
        assert_eq!(stored.finished(), 200);
    }
}
