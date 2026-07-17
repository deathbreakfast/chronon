//! BM-CH7D: production worker runtime claim+execute drain throughput.

use std::time::{Duration, Instant};

use anyhow::Result;
use chronon_testkit::{seed_due_cron_jobs, BootstrapSession, DeploymentKind, MatrixSpec, NOOP_SCRIPT};

use crate::report::BenchReport;
use crate::runners::ch7_common::{
    count_queued_runs, effective_claim_rate, insufficient_sample, prefill_runs, should_prefill,
};
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CH7D: prefill queue, spawn worker runtime fleet, measure drain to empty queue.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let prefill = ctx.bench.prefill_count;
    let worker_hosts = ctx.bench.worker_host_count.max(1);
    let client_index = ctx.bench.bench_client_index;
    let id_prefix = format!("c{client_index}");

    let mut matrix = ctx.matrix.clone();
    matrix.deployment = DeploymentKind::CoordinatorWorker;

    let mut session = BootstrapSession::new(matrix);
    session.install().await?;
    let store = session.store_dyn()?;

    seed_due_cron_jobs(store.as_ref(), 1, NOOP_SCRIPT).await?;

    let prefill_elapsed = if should_prefill(&ctx.bench) {
        Some(prefill_runs(store.clone(), &ctx.bench, &id_prefix).await?)
    } else {
        None
    };

    session.spawn_workers_n(worker_hosts).await?;

    let drain_start = Instant::now();
    let timeout = Duration::from_secs(
        std::env::var("CHRONON_CH7D_DRAIN_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600),
    );
    let deadline = drain_start + timeout;

    while Instant::now() < deadline {
        if count_queued_runs(store.as_ref()).await? == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let queued_left = count_queued_runs(store.as_ref()).await?;
    session.shutdown_embedded().await?;

    let drain_elapsed = drain_start.elapsed().as_secs_f64().max(f64::EPSILON);
    let completed = prefill.saturating_sub(queued_left as u64);
    let ops_per_sec = completed as f64 / drain_elapsed;
    let effective_rate = effective_claim_rate(completed, drain_elapsed);

    let mut report = BenchReport::base(&ctx.plan.id, &MatrixSpec {
        deployment: DeploymentKind::CoordinatorWorker,
        ..ctx.matrix.clone()
    });
    report.metric_kind = Some("claim_execute".into());
    report.aggregate = Some(false);
    report.storage_topology.clone_from(&ctx.bench.storage_topology);
    report.tier_tag.clone_from(&ctx.bench.tier_tag);
    report.data_tier_profile.clone_from(&ctx.bench.data_tier_profile);
    report.prefill_elapsed_secs = prefill_elapsed;
    report.drain_elapsed_secs = Some(drain_elapsed);
    report.effective_drain_secs = Some(drain_elapsed);
    report.ops = Some(completed as usize);
    report.jobs = Some(prefill as usize);
    report.claim_ops_per_sec = Some(MetricStats {
        count: completed as usize,
        p50: ops_per_sec,
        p95: ops_per_sec,
        p99: ops_per_sec,
        min: ops_per_sec,
        max: ops_per_sec,
    });
    report.effective_claim_ops_per_sec = Some(MetricStats {
        count: completed as usize,
        p50: effective_rate,
        p95: effective_rate,
        p99: effective_rate,
        min: effective_rate,
        max: effective_rate,
    });
    if queued_left > 0 {
        report.status = "fail".into();
        report.error = Some(format!("{queued_left} runs still queued after drain window"));
    } else if insufficient_sample(drain_elapsed) {
        report.verdict = Some("insufficient_sample".into());
    }
    report.pass_notes = Some(format!(
        "drained {completed}/{prefill} runs in {drain_elapsed:.3}s ({ops_per_sec:.1}/s) \
         with {worker_hosts} worker hosts (client {client_index}/{})",
        ctx.bench.bench_client_count
    ));
    Ok(report)
}
