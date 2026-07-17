//! Bench run configuration — public knobs for capacity and sweep dimensions.
//!
//! Callers construct [`BenchRunConfig`] via [`BenchRunConfig::for_experiment`] or
//! [`crate::cli::bench_config::resolve_bench_config`].

use serde::{Deserialize, Serialize};

/// Worker pool layout for BM-CH7 D1 sweeps.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PoolLayout {
    /// All workers may claim from any pool (default).
    #[default]
    Shared,
    /// Each worker pinned to one pool shard (`general-{i % K}`).
    Distinct,
}

/// Sweep dimensions recorded in benchmark reports.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SweepDimensions {
    /// Parallel worker count (BM-CH7 S0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_count: Option<u32>,
    /// Seeded job count (BM-CH1 S1, BM-CHL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_count: Option<usize>,
    /// Partition count (BM-CH1/CH3 S3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_count: Option<u32>,
    /// Prefill run count before claim drain (BM-CH7 S4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill_count: Option<u64>,
    /// Multibench client index (S5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bench_client_index: Option<u32>,
    /// Multibench client count (S5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bench_client_count: Option<u32>,
    /// Worker pool shard count (BM-CH7 D1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_count: Option<u32>,
    /// Pool layout (`shared` | `distinct`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_layout: Option<PoolLayout>,
    /// Worker daemon host count (BM-CH7D D4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_host_count: Option<u32>,
    /// Fleet-wide concurrent claim loops (`bc × worker_count` when symmetric).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_concurrent_claimers: Option<u32>,
    /// Data-tier topology label (BM-CH7 D2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_topology: Option<String>,
    /// Campaign tier tag (D5 ladder T0–T7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_tag: Option<String>,
    /// Data tier profile (hardware/topology label).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_tier_profile: Option<String>,
}

/// Resolved configuration for one benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchRunConfig {
    /// Parallel workers for claim capacity (BM-CH7).
    pub worker_count: u32,
    /// Seeded jobs for query and sustain experiments.
    pub job_count: usize,
    /// Partition count for query and churn experiments.
    pub partition_count: u32,
    /// Runs pre-filled before claim drain (BM-CH7).
    pub prefill_count: u64,
    /// Multibench client index for disjoint worker IDs (S5).
    pub bench_client_index: u32,
    /// Multibench client count (S5).
    pub bench_client_count: u32,
    /// Worker pool count for claim sharding (D1).
    pub pool_count: u32,
    /// Pool layout for BM-CH7 D1.
    pub pool_layout: PoolLayout,
    /// Worker daemon host count for BM-CH7D (D4).
    pub worker_host_count: u32,
    /// Data-tier topology label written into reports (D2).
    pub storage_topology: Option<String>,
    /// Campaign tier tag (D5).
    pub tier_tag: Option<String>,
    /// Data tier profile label.
    pub data_tier_profile: Option<String>,
    /// Max due jobs returned per query/tick batch.
    pub tick_batch_limit: u32,
}

impl Default for BenchRunConfig {
    fn default() -> Self {
        Self {
            worker_count: 32,
            job_count: 1000,
            partition_count: 4,
            prefill_count: 10_000,
            bench_client_index: 0,
            bench_client_count: 1,
            pool_count: 1,
            pool_layout: PoolLayout::Shared,
            worker_host_count: 1,
            storage_topology: None,
            tier_tag: None,
            data_tier_profile: None,
            tick_batch_limit: 500,
        }
    }
}

impl BenchRunConfig {
    /// Defaults tuned per BM-CH* / BM-CHL* experiment id.
    #[must_use]
    pub fn for_experiment(experiment_id: &str) -> Self {
        let id = experiment_id.to_ascii_lowercase();
        let mut cfg = Self::default();

        match id.as_str() {
            "bm-ch1" => {
                cfg.job_count = 1000;
                cfg.partition_count = 4;
            }
            "bm-ch3" => {
                cfg.partition_count = 4;
            }
            "bm-ch7" => {
                cfg.worker_count = 32;
                cfg.prefill_count = 10_000;
            }
            "bm-ch7d" => {
                cfg.worker_count = 32;
                cfg.prefill_count = 100_000;
                cfg.worker_host_count = 1;
            }
            "bm-chl0" => {
                cfg.job_count = 10;
            }
            "bm-chl1" => {
                cfg.job_count = 100;
            }
            "bm-chl2" => {
                cfg.job_count = 1000;
            }
            "bm-chl3" => {
                cfg.job_count = 10_000;
            }
            _ => {}
        }

        cfg
    }

    /// Snapshot sweep dimensions for JSON reports.
    #[must_use]
    pub fn sweep_dimensions(&self) -> SweepDimensions {
        SweepDimensions {
            worker_count: Some(self.worker_count),
            job_count: Some(self.job_count),
            partition_count: Some(self.partition_count),
            prefill_count: Some(self.prefill_count),
            bench_client_index: Some(self.bench_client_index),
            bench_client_count: Some(self.bench_client_count),
            pool_count: Some(self.pool_count),
            pool_layout: Some(self.pool_layout),
            worker_host_count: Some(self.worker_host_count),
            total_concurrent_claimers: Some(
                self.bench_client_count.max(1) * self.worker_count.max(1),
            ),
            storage_topology: self.storage_topology.clone(),
            tier_tag: self.tier_tag.clone(),
            data_tier_profile: self.data_tier_profile.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BenchRunConfig;

    #[test]
    fn ch7_defaults_prefill_and_workers() {
        let cfg = BenchRunConfig::for_experiment("bm-ch7");
        assert_eq!(cfg.worker_count, 32);
        assert_eq!(cfg.prefill_count, 10_000);
    }

    #[test]
    fn ch7d_defaults_higher_prefill() {
        let cfg = BenchRunConfig::for_experiment("bm-ch7d");
        assert_eq!(cfg.prefill_count, 100_000);
    }

    #[test]
    fn chl_tiers_match_job_count() {
        assert_eq!(BenchRunConfig::for_experiment("bm-chl2").job_count, 1000);
        assert_eq!(BenchRunConfig::for_experiment("bm-chl3").job_count, 10_000);
    }
}
