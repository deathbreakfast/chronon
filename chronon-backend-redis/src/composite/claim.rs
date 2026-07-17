//! Redis-backed run claim loop for the composite store.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use chronon_backend_sql_common::run_pool_key;
use chronon_core::models::{Run, RunStatus};
use chronon_core::store::SchedulerStore;
use chronon_core::Result;

use crate::queue::RedisQueueLayer;

fn claim_batch_size() -> usize {
    std::env::var("CHRONON_CLAIM_BATCH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 64)
}

async fn requeue_mismatch(
    sql: &Arc<dyn SchedulerStore>,
    redis: &RedisQueueLayer,
    run_id: &str,
    pool_id: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let Some(run) = sql.get_run(run_id).await? else {
        return Ok(());
    };
    if run.status != RunStatus::Queued {
        return Ok(());
    }
    // Future-dated runs must stay in Redis until due; never drop them after a pop.
    if run.scheduled_for > now {
        redis
            .enqueue_run(
                run_pool_key(run.pool_id.as_deref()),
                &run.run_id,
                run.scheduled_for,
            )
            .await?;
        return Ok(());
    }
    let actual_pool = run_pool_key(run.pool_id.as_deref());
    if actual_pool != pool_id {
        redis
            .enqueue_run(actual_pool, &run.run_id, run.scheduled_for)
            .await?;
    } else if run.claim_lease_until.is_some_and(|u| u > now) {
        redis
            .enqueue_run(pool_id, &run.run_id, run.scheduled_for)
            .await?;
    }
    Ok(())
}

async fn claim_one(
    sql: &Arc<dyn SchedulerStore>,
    redis: &RedisQueueLayer,
    run_id: &str,
    pool_id: &str,
    worker_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<Option<Run>> {
    if let Some(run) = sql
        .claim_run_by_id(run_id, pool_id, worker_id, now, lease_ttl_secs)
        .await?
    {
        return Ok(Some(run));
    }
    requeue_mismatch(sql, redis, run_id, pool_id, now).await?;
    Ok(None)
}

async fn reconcile_unclaimed(
    sql: &Arc<dyn SchedulerStore>,
    redis: &RedisQueueLayer,
    run_ids: &[String],
    claimed: &[Run],
    pool_id: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let claimed_ids: HashSet<&str> = claimed.iter().map(|r| r.run_id.as_str()).collect();
    for run_id in run_ids {
        if !claimed_ids.contains(run_id.as_str()) {
            requeue_mismatch(sql, redis, run_id, pool_id, now).await?;
        }
    }
    Ok(())
}

/// Claim the next queued run via Redis ordering, validating against SQL state.
pub async fn claim_next_queued(
    sql: &Arc<dyn SchedulerStore>,
    redis: &RedisQueueLayer,
    pool_id: &str,
    worker_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<Option<Run>> {
    // Cap spin attempts so a persistent SQL/Redis mismatch cannot hang the worker.
    const MAX_ATTEMPTS: usize = 64;
    let batch = claim_batch_size();
    if batch <= 1 {
        for _ in 0..MAX_ATTEMPTS {
            let Some(run_id) = redis.claim_next_run_id(pool_id, now).await? else {
                return Ok(None);
            };
            if let Some(run) =
                claim_one(sql, redis, &run_id, pool_id, worker_id, now, lease_ttl_secs).await?
            {
                return Ok(Some(run));
            }
        }
        return Ok(None);
    }

    for _ in 0..MAX_ATTEMPTS {
        let run_ids = redis.claim_next_run_ids(pool_id, batch, now).await?;
        if run_ids.is_empty() {
            return Ok(None);
        }
        let id_refs: Vec<&str> = run_ids.iter().map(String::as_str).collect();
        let claimed = sql
            .claim_runs_by_ids(&id_refs, pool_id, worker_id, now, lease_ttl_secs)
            .await?;
        reconcile_unclaimed(sql, redis, &run_ids, &claimed, pool_id, now).await?;
        if let Some(run) = claimed.into_iter().next() {
            return Ok(Some(run));
        }
    }
    Ok(None)
}
