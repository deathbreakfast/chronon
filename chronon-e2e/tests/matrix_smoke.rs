//! Matrix smoke — bootstrap + scenario JSON roundtrip.

use chronon_testkit::{
    embedded_catalog, coordinator_catalog, run_catalog_entry, BootstrapSession, MatrixSpec,
    ScenarioSpec, StorageAdapter,
};

#[tokio::test]
async fn matrix_ci_mem_embedded_bootstrap_installs() {
    let mut session = BootstrapSession::new(MatrixSpec::ci_mem_embedded());
    session.install().await.expect("mem bootstrap");
    assert!(session.is_ready());
}

#[test]
fn scenario_scheduler_tick_smoke_spec_roundtrips_json() {
    let spec = ScenarioSpec::scheduler_tick_smoke();
    let json = serde_json::to_string(&spec).expect("serialize");
    let back: ScenarioSpec = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(spec, back);
}

/// Local full-catalog pass (not CI — individual scenario tests cover PR CI).
#[tokio::test]
#[ignore = "local full catalog pass — run with: cargo test -p chronon-e2e embedded_catalog_runs_sequentially -- --ignored"]
async fn embedded_catalog_runs_sequentially() {
    for entry in embedded_catalog() {
        run_catalog_entry(entry, StorageAdapter::Mem).await;
    }
}

/// Local full-catalog pass (not CI — individual scenario tests cover PR CI).
#[tokio::test]
#[ignore = "local full catalog pass — run with: cargo test -p chronon-e2e coordinator_catalog_runs_sequentially -- --ignored"]
async fn coordinator_catalog_runs_sequentially() {
    for entry in coordinator_catalog() {
        run_catalog_entry(entry, StorageAdapter::Mem).await;
    }
}
