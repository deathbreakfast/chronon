use std::time::Instant;

use anyhow::Result;
use chronon_scheduler::{tick_interval_ms_from_env, try_acquire_leader};
use chronon_testkit::BootstrapSession;
use tokio::time::{sleep, Duration};

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CH4: measure leader failover recovery within tick-interval budget.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let ttl_secs = 1_i64;
    std::env::set_var("CHRONON_LEADER_TTL_S", ttl_secs.to_string());
    let lease_wait = Duration::from_millis((ttl_secs * 1000 + 100) as u64);

    let mut session = BootstrapSession::new(ctx.matrix.clone());
    session.install().await?;
    session.init_partitions().await?;
    let store = session.store_dyn()?;

    let mut failover_samples = Vec::with_capacity(ctx.plan.default_ops);

    for _ in 0..ctx.plan.default_ops {
        assert!(try_acquire_leader(&store, "bench-leader-a").await?);

        sleep(lease_wait).await;

        let start = Instant::now();
        assert!(try_acquire_leader(&store, "bench-leader-b").await?);
        let _ = session.tick_once().await?;
        failover_samples.push(start.elapsed().as_secs_f64() * 1000.0);

        // Let bench-leader-b's lease expire before the next failover round.
        sleep(lease_wait).await;
    }

    let stats = MetricStats::summarize(failover_samples);
    let tick_budget_ms = tick_interval_ms_from_env() as f64 * 2.0;
    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.ops = Some(ctx.plan.default_ops);
    report.failover_ms = Some(stats);
    report.pass_notes = Some(format!(
        "failover p95 {:.3} ms (budget {:.0} ms = 2× tick interval)",
        stats.p95, tick_budget_ms
    ));
    Ok(report)
}
