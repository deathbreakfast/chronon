//! BM-CH-embed-burst: midnight due-job wave plus a colliding recurring cohort.
//!
//! Seeds due cron jobs, lets the embedded scheduler tick and workers execute the
//! registered sleep probe, then waits through the next five-minute analog firing.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use chronon_core::models::{Run, RunStatus};
use chronon_core::store::SchedulerStore;
use chronon_testkit::{
    smoke_actor_json, upsert_immediate_cron_job, BootstrapSession, DeploymentKind, FAIL_SCRIPT,
    SLEEP_SCRIPT,
};
use serde_json::json;

use crate::config::BenchRunConfig;
use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

const MAX_BURST_JOBS: usize = 10_000;
const FAST_BURST_THRESHOLD: usize = 32;
const DEFAULT_BACKGROUND: usize = 60;
const FAST_BACKGROUND: usize = 6;

/// Validated burst + recurring cohort.
#[derive(Debug, Clone)]
pub struct BurstWorkload {
    /// One-shot midnight cohort size.
    pub burst_jobs: usize,
    /// Recurring jobs, split evenly across hourly / 15-minute / 5-minute crons.
    pub background_jobs: usize,
    /// Cron for the five-minute analog cohort.
    pub five_min_cron: String,
    /// Cron for the 15-minute analog cohort.
    pub fifteen_min_cron: String,
    /// Cron for the hourly analog cohort.
    pub hourly_cron: String,
    /// How long to wait after the first wave for the next five-minute fire.
    pub recurring_wait: Duration,
    /// Hard cap on drain observation.
    pub drain_timeout: Duration,
    /// Script registered on seeded jobs.
    pub script_name: &'static str,
}

impl BurstWorkload {
    /// Build from bench knobs. Small job counts use second-scale crons for local smoke.
    pub fn from_bench(bench: &BenchRunConfig) -> Result<Self> {
        let burst_jobs = bench.job_count;
        let background_jobs = if burst_jobs <= FAST_BURST_THRESHOLD {
            FAST_BACKGROUND
        } else {
            DEFAULT_BACKGROUND
        };
        Self::new(
            burst_jobs,
            background_jobs,
            burst_jobs <= FAST_BURST_THRESHOLD,
        )
    }

    /// Construct a validated workload.
    pub fn new(burst_jobs: usize, background_jobs: usize, fast: bool) -> Result<Self> {
        if burst_jobs == 0 {
            bail!("burst job count must be greater than zero");
        }
        if burst_jobs > MAX_BURST_JOBS {
            bail!("burst job count {burst_jobs} exceeds bound {MAX_BURST_JOBS}");
        }
        if background_jobs > 0 && !background_jobs.is_multiple_of(3) {
            bail!("background cohort {background_jobs} must be divisible by 3");
        }
        let (five_min_cron, fifteen_min_cron, hourly_cron, recurring_wait, drain_timeout) = if fast
        {
            (
                "*/2 * * * * *".to_string(),
                "*/4 * * * * *".to_string(),
                "*/8 * * * * *".to_string(),
                Duration::from_secs(5),
                Duration::from_secs(20),
            )
        } else {
            (
                "0 */5 * * * *".to_string(),
                "0 */15 * * * *".to_string(),
                "0 0 * * * *".to_string(),
                Duration::from_secs(330),
                Duration::from_secs(900),
            )
        };
        Ok(Self {
            burst_jobs,
            background_jobs,
            five_min_cron,
            fifteen_min_cron,
            hourly_cron,
            recurring_wait,
            drain_timeout,
            script_name: SLEEP_SCRIPT,
        })
    }

    /// Expected Success count: burst + first recurring wave + the next five-minute fire.
    #[must_use]
    pub fn expected_runs(&self) -> u64 {
        let five_min = (self.background_jobs / 3) as u64;
        self.burst_jobs as u64 + self.background_jobs as u64 + five_min
    }
}

/// 80% 100 ms, 15% 250 ms, remainder 500 ms. Remainder keeps the vector length exact.
pub fn duration_mix_ms(count: usize) -> Result<Vec<u64>> {
    if count == 0 {
        bail!("duration mix requires at least one job");
    }
    if count > MAX_BURST_JOBS {
        bail!("duration mix count {count} exceeds bound {MAX_BURST_JOBS}");
    }
    let n100 = count * 80 / 100;
    let n250 = count * 15 / 100;
    let n500 = count
        .checked_sub(n100)
        .and_then(|n| n.checked_sub(n250))
        .ok_or_else(|| anyhow!("impossible duration mix for count {count}"))?;
    let mut out = Vec::with_capacity(count);
    out.extend(std::iter::repeat_n(100u64, n100));
    out.extend(std::iter::repeat_n(250u64, n250));
    out.extend(std::iter::repeat_n(500u64, n500));
    Ok(out)
}

/// Burst names plus three recurring cohorts.
#[derive(Debug, Default)]
struct SeededNames {
    burst: Vec<String>,
    five_min: Vec<String>,
    fifteen_min: Vec<String>,
    hourly: Vec<String>,
}

impl SeededNames {
    fn all(&self) -> impl Iterator<Item = &String> {
        self.burst
            .iter()
            .chain(self.five_min.iter())
            .chain(self.fifteen_min.iter())
            .chain(self.hourly.iter())
    }
}

/// BM-CH-embed-burst runner.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    run_layout(ctx, BurstLayout::Embedded).await
}

/// Same midnight burst on coordinator-worker hosts (`bm-ch-fleet-burst`).
pub async fn run_fleet(ctx: &RunContext) -> Result<BenchReport> {
    run_layout(ctx, BurstLayout::Fleet).await
}

enum BurstLayout {
    Embedded,
    Fleet,
}

async fn run_layout(ctx: &RunContext, layout: BurstLayout) -> Result<BenchReport> {
    let mut workload = BurstWorkload::from_bench(&ctx.bench)?;
    if std::env::var("CHRONON_BENCH_BURST_FAIL_PROBE")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true"))
    {
        workload.script_name = FAIL_SCRIPT;
        workload.background_jobs = 0;
    }
    run_with_workload(ctx, workload, layout).await
}

async fn run_with_workload(
    ctx: &RunContext,
    workload: BurstWorkload,
    layout: BurstLayout,
) -> Result<BenchReport> {
    let _env = ConcurrencyGuard::apply(ctx.bench.worker_count, ctx.bench.tick_batch_limit);

    let mut matrix = ctx.matrix.clone();
    if matches!(layout, BurstLayout::Fleet) {
        matrix.deployment = DeploymentKind::CoordinatorWorker;
    }
    let mut session = BootstrapSession::new(matrix);
    session.install().await?;
    match layout {
        BurstLayout::Embedded => session.spawn_embedded().await?,
        BurstLayout::Fleet => {
            session
                .spawn_coordinator_worker_n(ctx.bench.worker_host_count.max(1))
                .await?;
        }
    }
    let store = session.store_dyn()?;

    let enqueue_started = Instant::now();
    let seeded = match seed_workload(store.as_ref(), &workload).await {
        Ok(names) => names,
        Err(err) => {
            let _ = session.shutdown_embedded().await;
            return fail_report(ctx, &workload, format!("seed failed: {err}"));
        }
    };
    let burst_enqueue_ms = enqueue_started.elapsed().as_secs_f64() * 1000.0;

    let drain_started = Instant::now();
    let wait_result = wait_for_expected(store.as_ref(), &workload, &seeded).await;
    let burst_drain_elapsed_secs = drain_started.elapsed().as_secs_f64().max(1e-9);

    let runs = match list_all_runs(store.as_ref()).await {
        Ok(runs) => runs,
        Err(err) => {
            let _ = session.shutdown_embedded().await;
            return fail_report(ctx, &workload, format!("list runs failed: {err}"));
        }
    };

    if let Err(err) = session.shutdown_embedded().await {
        return fail_report(ctx, &workload, format!("shutdown failed: {err}"));
    }

    Ok(build_report(
        ctx,
        &workload,
        &seeded,
        &runs,
        burst_enqueue_ms,
        burst_drain_elapsed_secs,
        wait_result.err().map(|e| e.to_string()),
    ))
}

async fn seed_workload(
    store: &dyn SchedulerStore,
    workload: &BurstWorkload,
) -> Result<SeededNames> {
    let mix = if workload.script_name == SLEEP_SCRIPT {
        duration_mix_ms(workload.burst_jobs)?
    } else {
        vec![0; workload.burst_jobs]
    };
    let mut seeded = SeededNames::default();
    let mut names = HashSet::new();

    for (i, sleep_ms) in mix.into_iter().enumerate() {
        let name = format!("burst-{i:04}");
        if !names.insert(name.clone()) {
            bail!("duplicate burst job name {name}");
        }
        let job_id =
            upsert_named(store, &name, workload.script_name, "0 * * * * *", sleep_ms).await?;
        seeded.burst.push(job_id);
    }

    let cohort = workload.background_jobs / 3;
    for i in 0..cohort {
        let name = format!("recur-5m-{i:04}");
        names.insert(name.clone());
        let job_id = upsert_named(
            store,
            &name,
            workload.script_name,
            &workload.five_min_cron,
            100,
        )
        .await?;
        seeded.five_min.push(job_id);
    }
    for i in 0..cohort {
        let name = format!("recur-15m-{i:04}");
        names.insert(name.clone());
        let job_id = upsert_named(
            store,
            &name,
            workload.script_name,
            &workload.fifteen_min_cron,
            100,
        )
        .await?;
        seeded.fifteen_min.push(job_id);
    }
    for i in 0..cohort {
        let name = format!("recur-1h-{i:04}");
        names.insert(name.clone());
        let job_id = upsert_named(
            store,
            &name,
            workload.script_name,
            &workload.hourly_cron,
            100,
        )
        .await?;
        seeded.hourly.push(job_id);
    }

    if names.len() != seeded.all().count() {
        bail!("seeded job names were not unique");
    }
    Ok(seeded)
}

async fn upsert_named(
    store: &dyn SchedulerStore,
    name: &str,
    script: &str,
    cron: &str,
    sleep_ms: u64,
) -> Result<String> {
    let mut job = upsert_immediate_cron_job(store, name, script, cron).await?;
    job.actor_json = smoke_actor_json();
    job.params_json = json!({ "sleep_ms": sleep_ms, "job_name": name });
    job.timeout_ms = Some(10_000);
    store.upsert_job(&job).await?;
    Ok(job.job_id)
}

async fn wait_for_expected(
    store: &dyn SchedulerStore,
    workload: &BurstWorkload,
    seeded: &SeededNames,
) -> Result<()> {
    let deadline = Instant::now() + workload.drain_timeout;
    let expected = if workload.script_name == FAIL_SCRIPT {
        0
    } else {
        workload.expected_runs()
    };
    let mut saw_first_wave_at: Option<Instant> = None;
    loop {
        if Instant::now() >= deadline {
            bail!("drain timeout before expected terminal runs");
        }
        let runs = list_all_runs(store).await?;
        let success = runs
            .iter()
            .filter(|r| r.status == RunStatus::Success)
            .count() as u64;
        let failed = runs
            .iter()
            .filter(|r| r.status == RunStatus::Failed || r.status == RunStatus::Timeout)
            .count();
        if workload.script_name == FAIL_SCRIPT {
            if failed >= seeded.burst.len() {
                return Ok(());
            }
        } else {
            let first_wave = workload.burst_jobs as u64 + workload.background_jobs as u64;
            if success >= first_wave && saw_first_wave_at.is_none() {
                saw_first_wave_at = Some(Instant::now());
            }
            if let Some(t0) = saw_first_wave_at {
                if t0.elapsed() >= workload.recurring_wait {
                    let missed = missed_five_min(&runs, seeded);
                    if success >= expected && missed == 0 {
                        return Ok(());
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn list_all_runs(store: &dyn SchedulerStore) -> Result<Vec<Run>> {
    let mut all = Vec::new();
    let mut offset = 0usize;
    loop {
        let batch = store
            .list_runs_filtered(None, None, offset, 500)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        if batch.is_empty() {
            break;
        }
        offset += batch.len();
        all.extend(batch);
        if offset > 50_000 {
            bail!("run listing exceeded bound");
        }
    }
    Ok(all)
}

fn missed_five_min(runs: &[Run], seeded: &SeededNames) -> u64 {
    let mut missed = 0u64;
    for job_id in &seeded.five_min {
        let count = runs
            .iter()
            .filter(|r| {
                r.job_id.as_deref() == Some(job_id.as_str()) && r.status == RunStatus::Success
            })
            .count();
        if count < 2 {
            missed += 1;
        }
    }
    missed
}

fn build_report(
    ctx: &RunContext,
    workload: &BurstWorkload,
    seeded: &SeededNames,
    runs: &[Run],
    burst_enqueue_ms: f64,
    burst_drain_elapsed_secs: f64,
    wait_error: Option<String>,
) -> BenchReport {
    let burst_success: Vec<&Run> = runs
        .iter()
        .filter(|r| r.status == RunStatus::Success && is_burst_run(r, seeded))
        .collect();
    let start_samples = latencies_ms(
        burst_success.iter().copied(),
        |r| r.scheduled_for,
        |r| r.started_at,
    );
    let terminal_samples = latencies_ms(
        burst_success.iter().copied(),
        |r| r.scheduled_for,
        |r| r.finished_at,
    );
    let recurring_success: Vec<&Run> = runs
        .iter()
        .filter(|r| r.status == RunStatus::Success && is_recurring_run(r, seeded))
        .collect();
    let lateness_samples = latencies_ms(
        recurring_success.iter().copied(),
        |r| r.scheduled_for,
        |r| r.started_at,
    );

    let successful_runs = runs
        .iter()
        .filter(|r| r.status == RunStatus::Success)
        .count() as u64;
    let expected = workload.expected_runs();
    let missed = missed_five_min(runs, seeded);
    let burst_completed = burst_success.len() as f64;
    let burst_rps = burst_completed / burst_drain_elapsed_secs;

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.jobs = Some(workload.burst_jobs);
    report.ops = Some(expected as usize);
    report.metric_kind = Some("burst_completion".into());
    report.burst_enqueue_ms = Some(burst_enqueue_ms);
    report.scheduled_to_start_ms = Some(MetricStats::summarize(start_samples));
    report.scheduled_to_terminal_ms = Some(MetricStats::summarize(terminal_samples));
    report.burst_drain_elapsed_secs = Some(burst_drain_elapsed_secs);
    report.burst_completed_runs_per_sec = Some(burst_rps);
    report.recurring_lateness_ms = Some(MetricStats::summarize(lateness_samples));
    report.expected_runs = Some(expected);
    report.successful_runs = Some(successful_runs);
    report.missed_recurring_runs = Some(missed);
    report.drain_elapsed_secs = Some(burst_drain_elapsed_secs);
    report.error_rate = Some(if expected == 0 {
        0.0
    } else {
        1.0 - (successful_runs.min(expected) as f64 / expected as f64)
    });

    let fail_probe = workload.script_name == FAIL_SCRIPT;
    let timed_out = wait_error.is_some();
    let pass = !fail_probe && !timed_out && successful_runs >= expected && missed == 0;
    if pass {
        report.status = "ok".into();
        report.pass_notes = Some(format!(
            "burst {burst_rps:.2} completed runs/s; sleep mix 80/15/5 of 100/250/500 ms; W={}",
            ctx.bench.worker_count
        ));
    } else {
        report.status = "fail".into();
        report.error = wait_error.or_else(|| {
            Some(format!(
                "expected {expected} successes, got {successful_runs}; missed_recurring={missed}"
            ))
        });
        report.pass_notes = Some("burst correctness gates failed".into());
    }
    report
}

fn is_burst_run(run: &Run, seeded: &SeededNames) -> bool {
    run.job_id
        .as_deref()
        .is_some_and(|id| seeded.burst.iter().any(|job_id| job_id == id))
}

fn is_recurring_run(run: &Run, seeded: &SeededNames) -> bool {
    run.job_id.as_deref().is_some_and(|id| {
        seeded
            .five_min
            .iter()
            .chain(seeded.fifteen_min.iter())
            .chain(seeded.hourly.iter())
            .any(|job_id| job_id == id)
    })
}

fn latencies_ms<'a, I, Start, End>(runs: I, start: Start, end: End) -> Vec<f64>
where
    I: Iterator<Item = &'a Run>,
    Start: Fn(&Run) -> DateTime<Utc>,
    End: Fn(&Run) -> Option<DateTime<Utc>>,
{
    runs.filter_map(|run| {
        let finished = end(run)?;
        let ms = (finished - start(run)).num_milliseconds();
        if ms < 0 {
            None
        } else {
            Some(ms as f64)
        }
    })
    .collect()
}

fn fail_report(ctx: &RunContext, workload: &BurstWorkload, error: String) -> Result<BenchReport> {
    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.status = "fail".into();
    report.error = Some(error);
    report.expected_runs = Some(workload.expected_runs());
    report.successful_runs = Some(0);
    report.jobs = Some(workload.burst_jobs);
    Ok(report)
}

struct ConcurrencyGuard {
    worker: Option<String>,
    exec: Option<String>,
    tick: Option<String>,
}

impl ConcurrencyGuard {
    fn apply(worker_count: u32, tick_batch: u32) -> Self {
        let worker = std::env::var("CHRONON_WORKER_CONCURRENCY").ok();
        let exec = std::env::var("CHRONON_EXECUTOR_CONCURRENCY").ok();
        let tick = std::env::var("CHRONON_TICK_BATCH_LIMIT").ok();
        std::env::set_var(
            "CHRONON_WORKER_CONCURRENCY",
            worker_count.max(1).to_string(),
        );
        std::env::set_var(
            "CHRONON_EXECUTOR_CONCURRENCY",
            worker_count.max(1).to_string(),
        );
        std::env::set_var("CHRONON_TICK_BATCH_LIMIT", tick_batch.max(1).to_string());
        Self { worker, exec, tick }
    }
}

impl Drop for ConcurrencyGuard {
    fn drop(&mut self) {
        restore("CHRONON_WORKER_CONCURRENCY", self.worker.as_deref());
        restore("CHRONON_EXECUTOR_CONCURRENCY", self.exec.as_deref());
        restore("CHRONON_TICK_BATCH_LIMIT", self.tick.as_deref());
    }
}

fn restore(key: &str, prev: Option<&str>) {
    match prev {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BenchRunConfig;
    use crate::experiments::resolve_experiment;
    use chronon_testkit::MatrixSpec;

    #[test]
    fn mix_500_is_80_15_5() {
        let mix = duration_mix_ms(500).unwrap();
        assert_eq!(mix.len(), 500);
        assert_eq!(mix.iter().filter(|&&ms| ms == 100).count(), 400);
        assert_eq!(mix.iter().filter(|&&ms| ms == 250).count(), 75);
        assert_eq!(mix.iter().filter(|&&ms| ms == 500).count(), 25);
    }

    #[test]
    fn mix_rejects_zero_and_overflow() {
        assert!(duration_mix_ms(0).is_err());
        assert!(duration_mix_ms(MAX_BURST_JOBS + 1).is_err());
    }

    #[test]
    fn workload_rejects_bad_background() {
        assert!(BurstWorkload::new(500, 61, false).is_err());
        assert!(BurstWorkload::new(0, 60, false).is_err());
    }

    #[test]
    fn expected_runs_counts_second_five_min_wave() {
        let w = BurstWorkload::new(500, 60, false).unwrap();
        assert_eq!(w.expected_runs(), 500 + 60 + 20);
    }

    #[test]
    fn unique_seed_names() {
        let mix = duration_mix_ms(12).unwrap();
        let mut names = HashSet::new();
        for i in 0..mix.len() {
            assert!(names.insert(format!("burst-{i:04}")));
        }
        for i in 0..2 {
            assert!(names.insert(format!("recur-5m-{i:04}")));
            assert!(names.insert(format!("recur-15m-{i:04}")));
            assert!(names.insert(format!("recur-1h-{i:04}")));
        }
        assert_eq!(names.len(), 18);
    }

    #[test]
    fn summarize_skips_missing_timestamps() {
        let samples = latencies_ms(std::iter::empty(), |r| r.scheduled_for, |r| r.started_at);
        let stats = MetricStats::summarize(samples);
        assert_eq!(stats.count, 0);
        assert!(stats.p95.abs() < f64::EPSILON);
    }

    fn mem_ctx(jobs: usize) -> RunContext {
        let plan = resolve_experiment("bm-ch-embed-burst", None, Some(jobs)).unwrap();
        let mut bench = BenchRunConfig::for_experiment("bm-ch-embed-burst");
        bench.job_count = jobs;
        bench.worker_count = 4;
        RunContext {
            matrix: MatrixSpec::default(),
            plan,
            warmup: 0,
            bench,
        }
    }

    static BURST_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_burst_tests() -> std::sync::MutexGuard<'static, ()> {
        BURST_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(fut)
    }

    #[test]
    fn mem_embedded_burst_reaches_expected_success() {
        let _guard = lock_burst_tests();
        let ctx = mem_ctx(8);
        let report = block_on(run(&ctx)).unwrap();
        assert_eq!(report.status, "ok", "{:?}", report.error);
        assert!(
            report.successful_runs.unwrap_or(0) >= report.expected_runs.unwrap_or(u64::MAX),
            "successful {:?} expected {:?}",
            report.successful_runs,
            report.expected_runs
        );
        assert_eq!(report.missed_recurring_runs, Some(0));
        assert!(report.burst_completed_runs_per_sec.unwrap_or(0.0) > 0.0);
        assert!(report.scheduled_to_start_ms.unwrap().count > 0);
        assert!(report.scheduled_to_terminal_ms.unwrap().count > 0);
        assert!(report.burst_enqueue_ms.unwrap() >= 0.0);
    }

    #[test]
    fn mem_fleet_burst_reaches_expected_success() {
        let _guard = lock_burst_tests();
        let plan = resolve_experiment("bm-ch-fleet-burst", None, Some(8)).unwrap();
        let mut bench = BenchRunConfig::for_experiment("bm-ch-fleet-burst");
        bench.job_count = 8;
        bench.worker_count = 2;
        bench.worker_host_count = 2;
        let ctx = RunContext {
            matrix: MatrixSpec::ci_mem_coordinator_worker(),
            plan,
            warmup: 0,
            bench,
        };
        let report = block_on(run_fleet(&ctx)).unwrap();
        assert_eq!(report.status, "ok", "{:?}", report.error);
        assert!(
            report.successful_runs.unwrap_or(0) >= report.expected_runs.unwrap_or(u64::MAX),
            "successful {:?} expected {:?}",
            report.successful_runs,
            report.expected_runs
        );
        assert_eq!(report.missed_recurring_runs, Some(0));
        assert!(report.burst_completed_runs_per_sec.unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn failing_probe_marks_report_failed() {
        let _guard = lock_burst_tests();
        let ctx = mem_ctx(4);
        let mut workload = BurstWorkload::from_bench(&ctx.bench).unwrap();
        workload.script_name = FAIL_SCRIPT;
        workload.background_jobs = 0;
        let report = block_on(run_with_workload(&ctx, workload, BurstLayout::Embedded)).unwrap();
        assert_eq!(report.status, "fail");
        assert!(report.error.is_some());
    }

    #[test]
    fn drain_timeout_fails_closed() {
        let _guard = lock_burst_tests();
        let ctx = mem_ctx(8);
        let mut workload = BurstWorkload::from_bench(&ctx.bench).unwrap();
        workload.drain_timeout = Duration::from_millis(1);
        workload.recurring_wait = Duration::from_millis(1);
        let report = block_on(run_with_workload(&ctx, workload, BurstLayout::Embedded)).unwrap();
        assert_eq!(report.status, "fail");
        assert!(report
            .error
            .as_deref()
            .is_some_and(|e| e.contains("timeout") || e.contains("expected")));
    }
}
