//! Shared BM-CH7 / BM-CH7D prefill, pool routing, and timing helpers.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::Utc;
use chronon_core::models::{Run, RunStatus};
use chronon_core::store::SchedulerStore;

use crate::config::BenchRunConfig;

const DRAIN_POLL_MS: u64 = 25;
const DEFAULT_DRAIN_IDLE_SECS: f64 = 2.0;
const DEFAULT_PREFILL_WAIT_SECS: f64 = 120.0;

const BULK_PREFILL_THRESHOLD: u64 = 100_000;
const BULK_BATCH: usize = 512;

/// Whether this client should prefill the queue.
#[must_use]
pub fn should_prefill(cfg: &BenchRunConfig) -> bool {
    let drain_only = env_truthy("CHRONON_BENCH_DRAIN_ONLY");
    if drain_only {
        return cfg.bench_client_index == 0;
    }
    if cfg.bench_client_count > 1 {
        return cfg.bench_client_index == 0;
    }
    true
}

/// Worker pool id for a worker task index.
#[must_use]
pub fn worker_pool_id(cfg: &BenchRunConfig, worker_index: u32) -> String {
    if cfg.pool_count <= 1 {
        return "general".into();
    }
    if is_multibench(cfg) && cfg.worker_count == 1 {
        return format!("general-{}", cfg.bench_client_index % cfg.pool_count);
    }
    let slot = worker_index % cfg.pool_count;
    format!("general-{slot}")
}

/// Pool id for prefill run `i` (round-robin across K pools).
#[must_use]
pub fn prefill_pool_id(cfg: &BenchRunConfig, run_index: u64) -> String {
    if cfg.pool_count <= 1 {
        return "general".into();
    }
    format!("general-{}", run_index % u64::from(cfg.pool_count))
}

/// Insert `prefill` queued runs; returns prefill wall time in seconds.
pub async fn prefill_runs(
    store: Arc<dyn SchedulerStore>,
    cfg: &BenchRunConfig,
    id_prefix: &str,
) -> Result<f64> {
    let prefill = cfg.prefill_count;
    let now = Utc::now();
    let start = Instant::now();

    if prefill >= BULK_PREFILL_THRESHOLD {
        prefill_bulk(&store, cfg, id_prefix, prefill, now).await?;
    } else {
        for i in 0..prefill {
            let mut run = Run::new(chronon_testkit::NOOP_SCRIPT, now);
            run.run_id = format!("{id_prefix}-prefill-{i}");
            run.pool_id = Some(prefill_pool_id(cfg, i));
            run.status = RunStatus::Queued;
            store.create_run(&run).await?;
        }
    }

    Ok(start.elapsed().as_secs_f64())
}

async fn prefill_bulk(
    store: &Arc<dyn SchedulerStore>,
    cfg: &BenchRunConfig,
    id_prefix: &str,
    prefill: u64,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    let mut i = 0_u64;
    while i < prefill {
        let end = (i + BULK_BATCH as u64).min(prefill);
        let mut batch = Vec::with_capacity((end - i) as usize);
        for idx in i..end {
            let mut run = Run::new(chronon_testkit::NOOP_SCRIPT, now);
            run.run_id = format!("{id_prefix}-prefill-{idx}");
            run.pool_id = Some(prefill_pool_id(cfg, idx));
            run.status = RunStatus::Queued;
            batch.push(run);
        }
        let mut handles = Vec::with_capacity(batch.len());
        for run in batch {
            let store = store.clone();
            handles.push(tokio::spawn(async move { store.create_run(&run).await }));
        }
        for handle in handles {
            handle.await??;
        }
        i = end;
    }
    Ok(())
}

/// Whether this run participates in a multibench fleet (bc > 1).
#[must_use]
pub fn is_multibench(cfg: &BenchRunConfig) -> bool {
    cfg.bench_client_count > 1
}

/// Seconds to wait for client 0 prefill before drain-only clients begin claiming.
#[must_use]
pub fn prefill_wait_timeout_secs(cfg: &BenchRunConfig) -> f64 {
    env_f64("CHRONON_CH7_PREFILL_WAIT_SECS")
        .unwrap_or_else(|| DEFAULT_PREFILL_WAIT_SECS.max(cfg.prefill_count as f64 / 500.0))
}

/// Continuous empty-queue duration before multibench drain workers exit.
#[must_use]
pub fn drain_idle_secs() -> f64 {
    env_f64("CHRONON_CH7_DRAIN_IDLE_SECS").unwrap_or(DEFAULT_DRAIN_IDLE_SECS)
}

/// Whether this client only drains (multibench indices 1..bc-1).
#[must_use]
pub fn is_drain_only_client(cfg: &BenchRunConfig) -> bool {
    is_multibench(cfg) && cfg.bench_client_index != 0
}

/// Poll until at least `min_count` queued runs exist or `timeout_secs` elapses.
pub async fn wait_for_queued_runs(
    store: &dyn SchedulerStore,
    min_count: usize,
    timeout_secs: f64,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_secs);
    while Instant::now() < deadline {
        if count_queued_runs(store).await? >= min_count {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(DRAIN_POLL_MS)).await;
    }
    bail!(
        "timed out after {timeout_secs:.1}s waiting for >= {min_count} queued runs"
    );
}

/// Drain one worker against `pool_id`, polling until idle when multibench (bc > 1).
pub async fn drain_queued_with_idle_exit(
    store: Arc<dyn SchedulerStore>,
    cfg: &BenchRunConfig,
    pool_id: &str,
    worker_id: &str,
) -> Result<u64> {
    let multibench = is_multibench(cfg);
    let aws_drain_only = env_truthy("CHRONON_BENCH_DRAIN_ONLY");
    let idle_secs = drain_idle_secs();
    let prefill_wait = prefill_wait_timeout_secs(cfg);
    let started = Instant::now();
    let mut claimed = 0_u64;
    let mut idle_since: Option<Instant> = None;

    loop {
        match store
            .claim_next_queued(pool_id, worker_id, Utc::now(), 30)
            .await
        {
            Ok(Some(_)) => {
                claimed += 1;
                idle_since = None;
                if multibench {
                    tokio::task::yield_now().await;
                }
            }
            Ok(None) if !multibench => break,
            Ok(None) => {
                // Shared-queue multibench: another client may still be writing/claiming.
                // Partitioned pools (K>1): claim None means *this* pool is empty — do not
                // wait on sibling pools or unclaimable leftovers (that livelocks drainers).
                if cfg.pool_count <= 1 {
                    let queued = count_queued_runs(store.as_ref()).await?;
                    if queued > 0 {
                        idle_since = None;
                        tokio::time::sleep(Duration::from_millis(DRAIN_POLL_MS)).await;
                        continue;
                    }
                }
                if aws_drain_only && claimed == 0 {
                    if started.elapsed().as_secs_f64() >= prefill_wait {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(DRAIN_POLL_MS)).await;
                    continue;
                }
                let now = Instant::now();
                match idle_since {
                    None => idle_since = Some(now),
                    Some(start) if now.duration_since(start).as_secs_f64() >= idle_secs => break,
                    Some(_) => {}
                }
                tokio::time::sleep(Duration::from_millis(DRAIN_POLL_MS)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(claimed)
}

/// Prefill (if leader) or wait for queue (if drain-only multibench client).
pub async fn prepare_multibench_client(
    store: Arc<dyn SchedulerStore>,
    cfg: &BenchRunConfig,
) -> Result<Option<f64>> {
    let id_prefix = format!("c{}", cfg.bench_client_index);
    if should_prefill(cfg) {
        Ok(Some(prefill_runs(store, cfg, &id_prefix).await?))
    } else {
        Ok(None)
    }
}

/// Spawn `worker_count` drain tasks; returns total claims.
pub async fn run_drain_workers(
    store: Arc<dyn SchedulerStore>,
    cfg: &BenchRunConfig,
) -> Result<u64> {
    let client_index = cfg.bench_client_index;
    let worker_count = cfg.worker_count.max(1);
    let id_prefix = format!("c{client_index}");

    let mut handles = Vec::with_capacity(worker_count as usize);
    for w in 0..worker_count {
        let store = store.clone();
        let cfg = cfg.clone();
        let worker_id = format!("{id_prefix}-worker-{w}");
        let pool_id = worker_pool_id(&cfg, w);
        handles.push(tokio::spawn(async move {
            drain_queued_with_idle_exit(store, &cfg, &pool_id, &worker_id).await
        }));
    }

    let mut total = 0_u64;
    for handle in handles {
        total += handle.await??;
    }
    Ok(total)
}

/// Prefill (if leader), wait for queue (if drain-only), then drain with `worker_count` tasks.
pub async fn run_client_claim_drain(
    store: Arc<dyn SchedulerStore>,
    cfg: &BenchRunConfig,
) -> Result<u64> {
    prepare_multibench_client(store.clone(), cfg).await?;
    run_drain_workers(store, cfg).await
}

/// Count queued runs visible to the store (paginated scan).
pub async fn count_queued_runs(store: &dyn SchedulerStore) -> Result<usize> {
    let mut total = 0_usize;
    let mut offset = 0_usize;
    loop {
        let page = store
            .list_runs_filtered(None, Some(RunStatus::Queued), offset, 500)
            .await?;
        let n = page.len();
        total += n;
        if n < 500 {
            break;
        }
        offset += n;
    }
    Ok(total)
}

/// Effective claims/s after optional warmup trim.
#[must_use]
pub fn effective_claim_rate(claimed: u64, drain_secs: f64) -> f64 {
    let warmup = env_f64("CHRONON_CH7_WARMUP_SECS").unwrap_or(5.0);
    if drain_secs > warmup + 10.0 {
        claimed as f64 / (drain_secs - warmup).max(f64::EPSILON)
    } else {
        claimed as f64 / drain_secs.max(f64::EPSILON)
    }
}

/// Whether drain duration is too short for a reliable throughput sample.
#[must_use]
pub fn insufficient_sample(drain_secs: f64) -> bool {
    drain_secs < 10.0
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1" | "true" | "TRUE")
    )
}

fn env_f64(name: &str) -> Option<f64> {
    std::env::var(name).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{
        drain_idle_secs, is_multibench, prefill_pool_id, prefill_runs, prefill_wait_timeout_secs,
        run_drain_workers, should_prefill, worker_pool_id,
    };
    use crate::config::{BenchRunConfig, PoolLayout};

    #[test]
    fn multibench_only_client_zero_prefills() {
        let cfg = BenchRunConfig {
            bench_client_count: 4,
            bench_client_index: 1,
            ..BenchRunConfig::default()
        };
        assert!(!should_prefill(&cfg));
        let cfg = BenchRunConfig {
            bench_client_count: 4,
            bench_client_index: 0,
            ..BenchRunConfig::default()
        };
        assert!(should_prefill(&cfg));
    }

    #[test]
    fn distinct_pools_round_robin() {
        let cfg = BenchRunConfig {
            pool_count: 4,
            pool_layout: PoolLayout::Distinct,
            ..BenchRunConfig::default()
        };
        assert_eq!(prefill_pool_id(&cfg, 0), "general-0");
        assert_eq!(prefill_pool_id(&cfg, 5), "general-1");
        assert_eq!(worker_pool_id(&cfg, 3), "general-3");
    }

    #[test]
    fn multibench_w1_pins_pool_to_client_index() {
        let cfg = BenchRunConfig {
            bench_client_count: 4,
            bench_client_index: 2,
            worker_count: 1,
            pool_count: 4,
            ..BenchRunConfig::default()
        };
        assert_eq!(worker_pool_id(&cfg, 0), "general-2");
    }

    #[test]
    fn multibench_detected_when_bc_gt_one() {
        let cfg = BenchRunConfig {
            bench_client_count: 2,
            ..BenchRunConfig::default()
        };
        assert!(is_multibench(&cfg));
        let cfg = BenchRunConfig::default();
        assert!(!is_multibench(&cfg));
    }

    #[test]
    fn prefill_wait_scales_with_queue_depth() {
        let cfg = BenchRunConfig {
            prefill_count: 100_000,
            ..BenchRunConfig::default()
        };
        assert!(prefill_wait_timeout_secs(&cfg) >= 200.0);
        let cfg = BenchRunConfig {
            prefill_count: 2,
            ..BenchRunConfig::default()
        };
        assert!((prefill_wait_timeout_secs(&cfg) - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn drain_idle_default_two_seconds() {
        std::env::remove_var("CHRONON_CH7_DRAIN_IDLE_SECS");
        assert!((drain_idle_secs() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn drain_only_client_detected_by_index() {
        let cfg = BenchRunConfig {
            bench_client_count: 2,
            bench_client_index: 1,
            ..BenchRunConfig::default()
        };
        assert!(super::is_drain_only_client(&cfg));
        let cfg = BenchRunConfig {
            bench_client_count: 2,
            bench_client_index: 0,
            ..BenchRunConfig::default()
        };
        assert!(!super::is_drain_only_client(&cfg));
    }

    #[tokio::test]
    async fn drain_only_client_claims_prefilled_queue() {
        use chronon_testkit::BootstrapSession;
        use chronon_testkit::MatrixSpec;

        std::env::set_var("CHRONON_CH7_DRAIN_IDLE_SECS", "0.05");
        let mut session = BootstrapSession::new(MatrixSpec::default());
        session.install().await.expect("install");
        let store = session.store_dyn().expect("store");

        let leader = BenchRunConfig {
            bench_client_count: 2,
            bench_client_index: 0,
            prefill_count: 50,
            worker_count: 1,
            pool_count: 1,
            ..BenchRunConfig::default()
        };
        let drain_only = BenchRunConfig {
            bench_client_index: 1,
            ..leader.clone()
        };
        prefill_runs(store.clone(), &leader, "c0")
            .await
            .expect("prefill");
        let claimed = run_drain_workers(store, &drain_only)
            .await
            .expect("drain");
        std::env::remove_var("CHRONON_CH7_DRAIN_IDLE_SECS");
        assert_eq!(claimed, 50);
    }
}
