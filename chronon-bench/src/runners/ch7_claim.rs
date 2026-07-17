//! BM-CH7: worker claim throughput (`claim_next_queued` capacity).

use std::time::Instant;

use anyhow::Result;
use chronon_testkit::{seed_due_cron_jobs, BootstrapSession, NOOP_SCRIPT};

use crate::report::BenchReport;
use crate::runners::ch7_common::{
    effective_claim_rate, is_multibench, prepare_multibench_client, run_drain_workers,
};
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CH7: concurrent workers draining a pre-filled run queue.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let worker_count = ctx.bench.worker_count.max(1);
    let prefill = ctx.bench.prefill_count;
    let client_index = ctx.bench.bench_client_index;

    let mut session = BootstrapSession::new(ctx.matrix.clone());
    if is_multibench(&ctx.bench) {
        let cell_id = std::env::var("CHRONON_BENCH_CELL_ID").unwrap_or_else(|_| {
            format!(
                "bc{}-w{}-q{}",
                ctx.bench.bench_client_count,
                ctx.bench.worker_count,
                ctx.bench.prefill_count
            )
        });
        let is_leader = ctx.bench.bench_client_index == 0;
        session
            .install_multibench_cell(&cell_id, is_leader)
            .await?;
    } else {
        session.install().await?;
    }
    let store = session.store_dyn()?;

    if (!is_multibench(&ctx.bench) || ctx.bench.bench_client_index == 0)
        && store.list_jobs().await?.is_empty()
    {
        seed_due_cron_jobs(store.as_ref(), 1, NOOP_SCRIPT).await?;
    }

    let prefill_elapsed = prepare_multibench_client(store.clone(), &ctx.bench).await?;

    let drain_start = Instant::now();
    let total_claimed = run_drain_workers(store, &ctx.bench).await?;
    let drain_elapsed = drain_start.elapsed().as_secs_f64().max(f64::EPSILON);
    let ops_per_sec = total_claimed as f64 / drain_elapsed;
    let effective_rate = effective_claim_rate(total_claimed, drain_elapsed);

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.metric_kind = Some("claim".into());
    report.aggregate = Some(false);
    report.storage_topology.clone_from(&ctx.bench.storage_topology);
    report.tier_tag.clone_from(&ctx.bench.tier_tag);
    report.data_tier_profile.clone_from(&ctx.bench.data_tier_profile);
    report.prefill_elapsed_secs = prefill_elapsed;
    report.drain_elapsed_secs = Some(drain_elapsed);
    report.effective_drain_secs = Some(if drain_elapsed > 15.0 {
        drain_elapsed - 5.0
    } else {
        drain_elapsed
    });
    report.ops = Some(total_claimed as usize);
    report.jobs = Some(prefill as usize);
    report.claim_ops_per_sec = Some(MetricStats {
        count: total_claimed as usize,
        p50: ops_per_sec,
        p95: ops_per_sec,
        p99: ops_per_sec,
        min: ops_per_sec,
        max: ops_per_sec,
    });
    report.effective_claim_ops_per_sec = Some(MetricStats {
        count: total_claimed as usize,
        p50: effective_rate,
        p95: effective_rate,
        p99: effective_rate,
        min: effective_rate,
        max: effective_rate,
    });
    if ctx.bench.bench_client_count <= 1 {
        report.fleet_wall_claim_ops_per_sec = Some(ops_per_sec);
    }
    report.pass_notes = Some(format!(
        "claimed {total_claimed} runs in {drain_elapsed:.3}s ({ops_per_sec:.1} claims/s, effective {effective_rate:.1}/s) \
         with {worker_count} workers (prefill {prefill}, client {client_index}/{})",
        ctx.bench.bench_client_count
    ));
    Ok(report)
}
