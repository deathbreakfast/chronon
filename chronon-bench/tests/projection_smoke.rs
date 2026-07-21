//! Projection curve integration tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::fs;
use std::path::Path;

use chronon_bench::projection::{ch1_job_curve, ch7_worker_curve, chl_sustain_curve};
use chronon_bench::report::BenchReport;
use chronon_bench::stats::MetricStats;
use chronon_testkit::MatrixSpec;

fn write_report(dir: &Path, name: &str, mut report: BenchReport) {
    report.hardware = "test-hw".into();
    report.storage = "mem".into();
    fs::write(
        dir.join(name),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[test]
fn ch7_worker_curve_reads_sweep_reports() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let matrix = MatrixSpec::default();

    let mut r8 = BenchReport::base("bm-ch7", &matrix);
    r8.sweep_dimensions = Some(chronon_bench::config::SweepDimensions {
        worker_count: Some(8),
        ..Default::default()
    });
    r8.claim_ops_per_sec = Some(MetricStats {
        count: 1,
        p50: 100.0,
        p95: 100.0,
        p99: 100.0,
        min: 100.0,
        max: 100.0,
    });
    write_report(&path, "bm-ch7-mem-w8.json", r8);

    let mut r32 = BenchReport::base("bm-ch7", &matrix);
    r32.sweep_dimensions = Some(chronon_bench::config::SweepDimensions {
        worker_count: Some(32),
        ..Default::default()
    });
    r32.claim_ops_per_sec = Some(MetricStats {
        count: 1,
        p50: 400.0,
        p95: 400.0,
        p99: 400.0,
        min: 400.0,
        max: 400.0,
    });
    write_report(&path, "bm-ch7-mem-w32.json", r32);

    ch7_worker_curve("mem", "test-hw", &path, None).unwrap();
    assert!(path
        .join("scaling-curve-ch7-workers-mem-test-hw.json")
        .exists());
}

#[test]
fn ch1_job_curve_reads_query_reports() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let matrix = MatrixSpec::default();

    let mut r = BenchReport::base("bm-ch1", &matrix);
    r.sweep_dimensions = Some(chronon_bench::config::SweepDimensions {
        job_count: Some(1000),
        ..Default::default()
    });
    r.query_ms = Some(MetricStats {
        count: 10,
        p50: 1.0,
        p95: 2.0,
        p99: 3.0,
        min: 0.5,
        max: 4.0,
    });
    write_report(&path, "bm-ch1-mem-j1k.json", r);

    ch1_job_curve("mem", "test-hw", &path, None).unwrap();
    assert!(path
        .join("scaling-curve-ch1-jobs-mem-test-hw.json")
        .exists());
}

#[test]
fn chl_sustain_curve_reads_tick_reports() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let matrix = MatrixSpec::default();

    let mut r = BenchReport::base("bm-chl0", &matrix);
    r.jobs = Some(10);
    r.tick_ms = Some(MetricStats {
        count: 50,
        p50: 100.0,
        p95: 200.0,
        p99: 300.0,
        min: 50.0,
        max: 400.0,
    });
    r.error_rate = Some(0.0);
    write_report(&path, "bm-chl0-mem.json", r);

    chl_sustain_curve("mem", "test-hw", &path, None).unwrap();
    assert!(path
        .join("scaling-curve-chl-sustain-mem-test-hw.json")
        .exists());
}
