//! Multibench BM-CH7 smoke — two simulated clients on mem storage.

#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;

use chronon_bench::cli::bench_config::{resolve_bench_config, BenchConfigOverrides};
use chronon_bench::experiments::resolve_experiment;
use chronon_bench::projection::{ch7_aggregate, ch7_multibench_curve};
use chronon_bench::runners::ch7_common::{prefill_runs, run_drain_workers};
use chronon_bench::runners::{run_experiment, RunContext};
use chronon_bench::stats::MetricStats;
use chronon_testkit::{seed_due_cron_jobs, BootstrapSession, MatrixSpec, NOOP_SCRIPT};
use tempfile::TempDir;
use tokio::sync::Barrier;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn bench_cfg(idx: u32, prefill: u64, workers: u32) -> chronon_bench::config::BenchRunConfig {
    resolve_bench_config(
        "bm-ch7",
        BenchConfigOverrides {
            worker_count: Some(workers),
            prefill_count: Some(prefill),
            bench_client_index: Some(idx),
            bench_client_count: Some(2),
            pool_count: Some(1),
            ..BenchConfigOverrides::default()
        },
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multibench_two_clients_mem_smoke() {
    {
        let _lock = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("CHRONON_CH7_DRAIN_IDLE_SECS", "0.05");
        std::env::set_var("CHRONON_BENCH_HARDWARE", "local");
    }
    let dir = TempDir::new().expect("tempdir");
    let reports = dir.path().join("reports");
    std::fs::create_dir_all(&reports).expect("mkdir");

    let mut session = BootstrapSession::new(MatrixSpec::default());
    session.install().await.expect("install");
    let store = session.store_dyn().expect("store");
    seed_due_cron_jobs(store.as_ref(), 1, NOOP_SCRIPT)
        .await
        .expect("seed");

    let prefill = 32_u64;
    let workers = 4_u32;
    let cfg0 = bench_cfg(0, prefill, workers);
    let cfg1 = bench_cfg(1, prefill, workers);

    prefill_runs(store.clone(), &cfg0, "c0")
        .await
        .expect("prefill");

    let plan =
        resolve_experiment("bm-ch7", Some(workers as usize), Some(prefill as usize)).expect("plan");

    let dims0 = cfg0.sweep_dimensions();
    let dims1 = cfg1.sweep_dimensions();

    let barrier = Arc::new(Barrier::new(2));
    let store0 = store.clone();
    let store1 = store.clone();
    let b0 = barrier.clone();
    let b1 = barrier.clone();
    let (ops0, ops1) = tokio::join!(
        async move {
            b0.wait().await;
            run_drain_workers(store0, &cfg0).await.expect("client0")
        },
        async move {
            b1.wait().await;
            run_drain_workers(store1, &cfg1).await.expect("client1")
        },
    );

    assert_eq!(ops0 + ops1, prefill, "fleet must drain full prefill");
    assert!(ops0 > 0, "client 0 must claim runs");
    assert!(ops1 > 0, "client 1 must claim runs");

    for (idx, ops, dims) in [(0, ops0, dims0), (1, ops1, dims1)] {
        let mut report = chronon_bench::report::BenchReport::base(&plan.id, &MatrixSpec::default());
        report.ops = Some(ops as usize);
        report.jobs = Some(prefill as usize);
        report.sweep_dimensions = Some(dims);
        report.claim_ops_per_sec = Some(MetricStats {
            count: ops as usize,
            p50: ops as f64,
            p95: ops as f64,
            p99: ops as f64,
            min: ops as f64,
            max: ops as f64,
        });
        report.hardware = "local".into();
        report.storage = "mem".into();
        let path = reports.join(format!("bm-ch7-bc2-i{idx}-mem-local.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&report).expect("json")).expect("write");
    }

    ch7_aggregate("mem", "local", &reports, "bm-ch7-bc2").expect("aggregate");
    ch7_multibench_curve("mem", "local", &reports, None).expect("curve");

    let aggregate_path = reports.join("bm-ch7-bc2-aggregate-bc2-mem-local.json");
    assert!(aggregate_path.exists(), "aggregate report missing");
    let agg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(aggregate_path).expect("read"))
            .expect("json");
    assert!(
        agg.get("fleet_claim_ops_per_sec")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
            > 0.0
    );

    std::env::remove_var("CHRONON_BENCH_HARDWARE");
    std::env::remove_var("CHRONON_CH7_DRAIN_IDLE_SECS");
}

#[tokio::test]
async fn multibench_run_experiment_single_client_still_works() {
    let bench = resolve_bench_config(
        "bm-ch7",
        BenchConfigOverrides {
            worker_count: Some(4),
            prefill_count: Some(50),
            bench_client_count: Some(1),
            ..BenchConfigOverrides::default()
        },
    );
    let plan = resolve_experiment("bm-ch7", Some(4), Some(50)).expect("plan");
    let ctx = RunContext {
        matrix: MatrixSpec::default(),
        plan,
        warmup: 0,
        bench,
    };
    let report = run_experiment(&ctx).await.expect("run");
    assert_eq!(report.ops, Some(50));
}

#[test]
fn multibench_cli_smoke_script_exists() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace")
        .to_path_buf();
    let script = root.join("chronon-bench/scripts/run-ch7-multibench-smoke.sh");
    assert!(script.exists(), "missing {}", script.display());
    let status = Command::new("bash")
        .arg("-n")
        .arg(&script)
        .status()
        .expect("bash -n");
    assert!(status.success());
}
