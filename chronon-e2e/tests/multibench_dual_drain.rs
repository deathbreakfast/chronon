//! E2E: two multibench clients drain a shared queue (BM-CH7 regression).

#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chronon_bench::cli::bench_config::{resolve_bench_config, BenchConfigOverrides};
use chronon_bench::runners::ch7_common::{prefill_runs, run_drain_workers};
use chronon_testkit::{seed_due_cron_jobs, BootstrapSession, MatrixSpec, NOOP_SCRIPT};
use tokio::sync::Barrier;
use tokio::time::{sleep, Duration};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn client_bench(index: u32, prefill: u64, workers: u32) -> chronon_bench::config::BenchRunConfig {
    resolve_bench_config(
        "bm-ch7",
        BenchConfigOverrides {
            worker_count: Some(workers),
            prefill_count: Some(prefill),
            bench_client_index: Some(index),
            bench_client_count: Some(2),
            pool_count: Some(1),
            ..BenchConfigOverrides::default()
        },
    )
}

fn client_bench_pooled(
    index: u32,
    prefill: u64,
    workers: u32,
    pools: u32,
) -> chronon_bench::config::BenchRunConfig {
    resolve_bench_config(
        "bm-ch7",
        BenchConfigOverrides {
            worker_count: Some(workers),
            prefill_count: Some(prefill),
            bench_client_index: Some(index),
            bench_client_count: Some(2),
            pool_count: Some(pools),
            ..BenchConfigOverrides::default()
        },
    )
}

#[tokio::test]
async fn multibench_dual_drain_shared_queue() {
    {
        let _lock = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("CHRONON_CH7_DRAIN_IDLE_SECS", "0.05");
        std::env::set_var("CHRONON_CH7_PREFILL_WAIT_SECS", "30");
    }

    let mut session = BootstrapSession::new(MatrixSpec::default());
    session.install().await.expect("install");
    let store = session.store_dyn().expect("store");
    seed_due_cron_jobs(store.as_ref(), 1, NOOP_SCRIPT)
        .await
        .expect("seed");

    let prefill = 50_u64;
    let cfg0 = client_bench(0, prefill, 1);
    let cfg1 = client_bench(1, prefill, 1);

    let store1 = store.clone();
    let drain_only = tokio::spawn(async move {
        let start = Instant::now();
        let ops = run_drain_workers(store1, &cfg1).await.expect("drain");
        (ops, start.elapsed())
    });

    sleep(Duration::from_millis(50)).await;
    prefill_runs(store.clone(), &cfg0, "c0")
        .await
        .expect("prefill");

    let (ops1, elapsed) = drain_only.await.expect("join");
    std::env::remove_var("CHRONON_CH7_DRAIN_IDLE_SECS");
    std::env::remove_var("CHRONON_CH7_PREFILL_WAIT_SECS");

    assert_eq!(ops1, prefill);
    assert!(
        elapsed >= Duration::from_millis(40),
        "drain-only client must wait for prefill, not exit instantly ({elapsed:?})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multibench_two_clients_split_prefilled_queue() {
    {
        let _lock = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("CHRONON_CH7_DRAIN_IDLE_SECS", "0.05");
    }

    let mut session = BootstrapSession::new(MatrixSpec::default());
    session.install().await.expect("install");
    let store = session.store_dyn().expect("store");
    seed_due_cron_jobs(store.as_ref(), 1, NOOP_SCRIPT)
        .await
        .expect("seed");

    // Two pools + W=1 routes each multibench client to its own general-{i} partition
    // (round-robin prefill), so both must claim without depending on scheduler fairness.
    let prefill = 40_u64;
    let workers = 1_u32;
    let pools = 2_u32;
    let cfg0 = client_bench_pooled(0, prefill, workers, pools);
    let cfg1 = client_bench_pooled(1, prefill, workers, pools);

    prefill_runs(store.clone(), &cfg0, "c0")
        .await
        .expect("prefill");

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

    std::env::remove_var("CHRONON_CH7_DRAIN_IDLE_SECS");

    assert_eq!(ops0 + ops1, prefill);
    assert_eq!(
        ops0,
        prefill / u64::from(pools),
        "client 0 pool share, got {ops0}"
    );
    assert_eq!(
        ops1,
        prefill / u64::from(pools),
        "client 1 pool share, got {ops1}"
    );
}
