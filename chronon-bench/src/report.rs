//! JSON report shape for benchmark runs.

use std::path::PathBuf;

use chrono::Utc;
use chronon_testkit::MatrixSpec;
use serde::{Deserialize, Serialize};

use crate::config::SweepDimensions;
use crate::stats::MetricStats;

/// JSON report emitted after each benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    /// Experiment id (for example `"bm-ch0"`).
    pub experiment: String,
    /// Concatenated matrix slug from [`MatrixSpec::report_slug`].
    pub matrix_slug: String,
    /// Hardware profile label (`CHRONON_BENCH_HARDWARE` or `"local"`).
    pub hardware: String,
    /// Storage adapter slug.
    pub storage: String,
    /// Deployment shape slug.
    pub deployment: String,
    /// Telemetry adapter slug.
    pub telemetry: String,
    /// Topology slug.
    pub topology: String,
    /// Optional scenario id when driven by testkit scenarios.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<String>,
    /// Measured operation count for this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<usize>,
    /// Seeded job count for load experiments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<usize>,
    /// Coordinator tick latency stats (milliseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_ms: Option<MetricStats>,
    /// Due-job query latency stats (milliseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_ms: Option<MetricStats>,
    /// Chronon cron evaluator throughput (evaluations per second).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_evals_per_sec: Option<f64>,
    /// Baseline croner throughput for comparison.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_baseline_evals_per_sec: Option<f64>,
    /// Partition reassignment latency during churn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reassign_ms: Option<MetricStats>,
    /// Tick latency while partitions are reassigning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_delay_ms: Option<MetricStats>,
    /// Leader failover recovery latency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failover_ms: Option<MetricStats>,
    /// Successful script runs per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs_per_sec: Option<f64>,
    /// Raw tokio spawn baseline runs per second (BM-CH5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokio_spawn_baseline_runs_per_sec: Option<f64>,
    /// Enqueue-to-run completion latency for coordinator-worker deployment (BM-CH6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enqueue_to_run_ms: Option<MetricStats>,
    /// Enqueue-to-run latency for embedded deployment (BM-CH6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded_enqueue_to_run_ms: Option<MetricStats>,
    /// Worker claim throughput (`claim_next_queued` ops/s) for BM-CH7.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_ops_per_sec: Option<MetricStats>,
    /// Warmup-trimmed claim throughput when drain exceeds 15s.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_claim_ops_per_sec: Option<MetricStats>,
    /// Metric track label (`claim` | `claim_execute`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_kind: Option<String>,
    /// Data-tier topology label (BM-CH7 D2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_topology: Option<String>,
    /// Campaign tier tag (D5 ladder).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_tag: Option<String>,
    /// Data tier profile label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_tier_profile: Option<String>,
    /// Whether this report is a multibench aggregate cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<bool>,
    /// Fleet aggregate claims/s for multibench cells.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_claim_ops_per_sec: Option<f64>,
    /// Wall-clock fleet claims/s (`sum(ops) / max(drain_elapsed)` across clients).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_wall_claim_ops_per_sec: Option<f64>,
    /// Multibench efficiency vs bc × bc1 peak.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multibench_efficiency: Option<f64>,
    /// Prefill phase duration in seconds (off hot path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_elapsed_secs: Option<f64>,
    /// Drain phase duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drain_elapsed_secs: Option<f64>,
    /// Effective drain window after warmup trim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_drain_secs: Option<f64>,
    /// Sweep knob values for this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sweep_dimensions: Option<SweepDimensions>,
    /// Optional verdict tag from pass evaluation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// Fraction of operations that failed (0.0–1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_rate: Option<f64>,
    /// Overall run status (`"ok"` or `"fail"`).
    pub status: String,
    /// Human-readable pass notes for dashboards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass_notes: Option<String>,
    /// Top-level error message when the run aborted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// RFC3339 timestamp when the report was recorded.
    pub recorded_at: String,
}

impl BenchReport {
    /// Read hardware profile from `CHRONON_BENCH_HARDWARE` or default `"local"`.
    pub fn hardware_profile() -> String {
        std::env::var("CHRONON_BENCH_HARDWARE").unwrap_or_else(|_| "local".into())
    }

    /// Base report header for an experiment and matrix row.
    pub fn base(experiment: &str, matrix: &MatrixSpec) -> Self {
        Self {
            experiment: experiment.to_string(),
            matrix_slug: matrix.report_slug(),
            hardware: Self::hardware_profile(),
            storage: matrix.storage.as_str().to_string(),
            deployment: matrix.deployment.as_str().to_string(),
            telemetry: matrix.telemetry.as_str().to_string(),
            topology: matrix.topology.as_str().to_string(),
            scenario_id: None,
            ops: None,
            jobs: None,
            tick_ms: None,
            query_ms: None,
            cron_evals_per_sec: None,
            cron_baseline_evals_per_sec: None,
            reassign_ms: None,
            tick_delay_ms: None,
            failover_ms: None,
            runs_per_sec: None,
            tokio_spawn_baseline_runs_per_sec: None,
            enqueue_to_run_ms: None,
            embedded_enqueue_to_run_ms: None,
            claim_ops_per_sec: None,
            effective_claim_ops_per_sec: None,
            metric_kind: None,
            storage_topology: None,
            tier_tag: None,
            data_tier_profile: None,
            aggregate: None,
            fleet_claim_ops_per_sec: None,
            fleet_wall_claim_ops_per_sec: None,
            multibench_efficiency: None,
            prefill_elapsed_secs: None,
            drain_elapsed_secs: None,
            effective_drain_secs: None,
            sweep_dimensions: None,
            verdict: None,
            error_rate: None,
            status: "ok".to_string(),
            pass_notes: None,
            error: None,
            recorded_at: Utc::now().to_rfc3339(),
        }
    }

    /// Default JSON path under `profiling/chronon-bench/reports/`.
    pub fn default_report_path(experiment: &str, matrix: &MatrixSpec) -> PathBuf {
        PathBuf::from(format!(
            "profiling/chronon-bench/reports/{}-{}-{}.json",
            experiment,
            matrix.report_slug(),
            Self::hardware_profile()
        ))
    }

    /// Filename for a matrix batch run.
    pub fn report_filename(experiment: &str, matrix_slug: &str, hardware: &str) -> String {
        format!("{experiment}-{matrix_slug}-{hardware}.json")
    }
}
