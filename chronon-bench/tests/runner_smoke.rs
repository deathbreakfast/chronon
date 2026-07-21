//! Integration smoke tests for benchmark runners (no external DB).

#![allow(clippy::unwrap_used, clippy::expect_used)]
use chronon_bench::cli::bench_config::{resolve_bench_config, BenchConfigOverrides};
use chronon_bench::config::BenchRunConfig;
use chronon_bench::experiments::resolve_experiment;
use chronon_bench::runners::{run_experiment, RunContext};
use chronon_testkit::MatrixSpec;

fn mem_matrix() -> MatrixSpec {
    MatrixSpec::default()
}

#[tokio::test]
async fn bm_ch2_produces_cron_throughput() {
    let plan = resolve_experiment("bm-ch2", Some(5000), None).unwrap();
    let ctx = RunContext {
        matrix: mem_matrix(),
        plan,
        warmup: 0,
        bench: BenchRunConfig::for_experiment("bm-ch2"),
    };
    let report = run_experiment(&ctx).await.unwrap();
    assert!(report.cron_evals_per_sec.unwrap_or(0.0) > 0.0);
    assert!(report.cron_baseline_evals_per_sec.unwrap_or(0.0) > 0.0);
    assert!(report.verdict.is_some());
}

#[tokio::test]
async fn bench_config_partition_override() {
    let cfg = resolve_bench_config(
        "bm-ch1",
        BenchConfigOverrides {
            partition_count: Some(16),
            job_count: Some(500),
            ..BenchConfigOverrides::default()
        },
    );
    assert_eq!(cfg.partition_count, 16);
    assert_eq!(cfg.job_count, 500);
}
