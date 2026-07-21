//! Multi-process distributed smoke: coordinator + two worker daemons against postgres-redis.
//!
//! Run when remote daemons are already up (`CHRONON_DISTRIBUTED_MODE=remote`) or spawn
//! local child processes (`CHRONON_DISTRIBUTED_MODE=local`, default).
//!
//! In-process postgres-redis dual-worker helpers also live here via
//! [`chronon_testkit::matrix_distributed_scenario_suite`].

#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use chronon_backend_postgres::{postgres_test_url, PostgresSchedulerStore};
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};
use chronon_core::models::{Job, Run, RunStatus, ScheduleKind};
use chronon_core::store::SchedulerStore;
use chronon_scheduler::partition_hash_i64_for_job_id;
use chronon_testkit::distributed_store_available;
use chronon_testkit::smoke_actor_json;
use tokio::process::{Child, Command};
use tokio::time::{sleep, Instant};

chronon_testkit::matrix_distributed_scenario_suite!();

fn redis_available() -> bool {
    distributed_store_available()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn daemon_bin(example: &str) -> PathBuf {
    workspace_root().join(format!("target/debug/examples/{example}"))
}

async fn connect_store(
    schema: &str,
    redis_prefix: &str,
) -> anyhow::Result<std::sync::Arc<dyn SchedulerStore>> {
    let pg_url = postgres_test_url();
    let redis_url =
        std::env::var("CHRONON_REDIS_URL").or_else(|_| std::env::var("CHRONON_TEST_REDIS_URL"))?;
    let sql = std::sync::Arc::new(PostgresSchedulerStore::connect_isolated(&pg_url, schema).await?);
    let redis = RedisQueueLayer::connect(&redis_url, Some(redis_prefix)).await?;
    Ok(std::sync::Arc::new(PostgresRedisSchedulerStore::new(
        sql, redis,
    )))
}

fn spawn_daemon(
    example: &str,
    instance_id: &str,
    pool: Option<&str>,
    schema: &str,
    redis_prefix: &str,
) -> anyhow::Result<Child> {
    let bin = daemon_bin(example);
    if !bin.exists() {
        anyhow::bail!(
            "missing daemon binary {} — run: cargo build -p uf-chronon --example {example} --features postgres,redis",
            bin.display()
        );
    }

    let mut cmd = Command::new(&bin);
    cmd.env("CHRONON_INSTANCE_ID", instance_id)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(pool_id) = pool {
        cmd.env("CHRONON_WORKER_POOL", pool_id);
    }
    if let Ok(url) = std::env::var("CHRONON_POSTGRES_URL") {
        cmd.env("CHRONON_POSTGRES_URL", url);
    }
    if let Ok(url) = std::env::var("CHRONON_REDIS_URL") {
        cmd.env("CHRONON_REDIS_URL", url);
    }
    if let Ok(url) = std::env::var("CHRONON_TEST_REDIS_URL") {
        cmd.env("CHRONON_TEST_REDIS_URL", url);
    }
    cmd.env("CHRONON_POSTGRES_SCHEMA", schema)
        .env("CHRONON_REDIS_PREFIX", redis_prefix)
        .env("CHRONON_POSTGRES_SKIP_BOOTSTRAP", "1");
    Ok(cmd.spawn()?)
}

async fn stop_child(mut child: Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn wait_for_terminal_runs(
    store: &dyn SchedulerStore,
    job_id: &str,
    prefill: usize,
    timeout: Duration,
) -> anyhow::Result<Vec<Run>> {
    let deadline = Instant::now() + timeout;
    loop {
        let runs = store.list_runs_for_job(job_id, 100).await?;
        let done: Vec<_> = runs
            .iter()
            .filter(|r| r.status == RunStatus::Success || r.status == RunStatus::Claimed)
            .cloned()
            .collect();
        if done.len() == prefill {
            return Ok(done);
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "expected {prefill} terminal runs, got {} after {:?} (statuses: {:?})",
                done.len(),
                timeout,
                runs.iter()
                    .map(|r| (&r.run_id, r.status))
                    .collect::<Vec<_>>()
            );
        }
        sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
#[ignore = "requires CHRONON_POSTGRES_URL and CHRONON_REDIS_URL — multi-process distributed smoke"]
async fn remote_dual_worker_no_double_claim() {
    if !redis_available() {
        return;
    }

    let isolate = uuid::Uuid::new_v4().simple().to_string();
    let schema = format!("chronon_dist_{isolate}");
    let redis_prefix = format!("chronon_dist_{isolate}");

    let store = connect_store(&schema, &redis_prefix).await.expect("store");

    let mode = std::env::var("CHRONON_DISTRIBUTED_MODE").unwrap_or_else(|_| "local".into());
    let mut children: Vec<Child> = Vec::new();
    if mode == "local" {
        children.push(
            spawn_daemon(
                "worker_daemon",
                "worker-a",
                Some("general"),
                &schema,
                &redis_prefix,
            )
            .expect("w-a"),
        );
        children.push(
            spawn_daemon(
                "worker_daemon",
                "worker-b",
                Some("general"),
                &schema,
                &redis_prefix,
            )
            .expect("w-b"),
        );
        sleep(Duration::from_secs(2)).await;
    }
    let job_name = format!("remote-dual-claim-{}", uuid::Uuid::new_v4().simple());
    let mut job = Job::new(&job_name, "daemon-noop");
    job.schedule_kind = ScheduleKind::Manual;
    job.actor_json = smoke_actor_json();
    job.partition_hash = Some(partition_hash_i64_for_job_id(&job.job_id));
    let job_id = job.job_id.clone();
    store.upsert_job(&job).await.expect("upsert job");

    let prefill = 6usize;
    let now = chrono::Utc::now();
    for i in 0..prefill {
        let mut run = Run::for_job(
            &job_id,
            "daemon-noop",
            now - chrono::Duration::seconds(i as i64),
        );
        run.pool_id = Some("general".into());
        store.create_run(&run).await.expect("create run");
    }

    let done = wait_for_terminal_runs(store.as_ref(), &job_id, prefill, Duration::from_secs(60))
        .await
        .expect("workers drain queue");

    let unique: HashSet<_> = done.iter().map(|r| &r.run_id).collect();
    assert_eq!(
        unique.len(),
        prefill,
        "duplicate claims across remote workers"
    );

    for child in children {
        stop_child(child).await;
    }
}
