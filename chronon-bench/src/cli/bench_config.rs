//! Resolve [`BenchRunConfig`] from experiment defaults plus caller overrides.
//!
//! Precedence: experiment defaults → `CHRONON_BENCH_*` env → CLI flags on `run`.

use crate::config::{BenchRunConfig, PoolLayout};

/// Optional CLI / harness overrides layered onto [`BenchRunConfig::for_experiment`].
#[derive(Debug, Clone, Default)]
pub struct BenchConfigOverrides {
    /// Parallel worker count (BM-CH7).
    pub worker_count: Option<u32>,
    /// Seeded job count.
    pub job_count: Option<usize>,
    /// Partition count.
    pub partition_count: Option<u32>,
    /// Prefill run count (BM-CH7).
    pub prefill_count: Option<u64>,
    /// Multibench client index.
    pub bench_client_index: Option<u32>,
    /// Multibench client count.
    pub bench_client_count: Option<u32>,
    /// Worker pool count (D1).
    pub pool_count: Option<u32>,
    /// Pool layout.
    pub pool_layout: Option<PoolLayout>,
    /// Worker daemon host count (D4).
    pub worker_host_count: Option<u32>,
    /// Data-tier topology label (D2).
    pub storage_topology: Option<String>,
    /// Tick/query batch limit.
    pub tick_batch_limit: Option<u32>,
}

/// Build the effective config for one run.
///
/// Precedence (lowest → highest): experiment defaults, harness env, CLI flags.
#[must_use]
pub fn resolve_bench_config(
    experiment_id: &str,
    overrides: BenchConfigOverrides,
) -> BenchRunConfig {
    let mut cfg = BenchRunConfig::for_experiment(experiment_id);
    apply_env_overrides(&mut cfg);
    apply_overrides(&mut cfg, overrides);
    cfg
}

fn apply_overrides(cfg: &mut BenchRunConfig, o: BenchConfigOverrides) {
    if let Some(w) = o.worker_count {
        cfg.worker_count = w.max(1);
    }
    if let Some(j) = o.job_count {
        cfg.job_count = j.max(1);
    }
    if let Some(p) = o.partition_count {
        cfg.partition_count = p.max(1);
    }
    if let Some(q) = o.prefill_count {
        cfg.prefill_count = q.max(1);
    }
    if let Some(i) = o.bench_client_index {
        cfg.bench_client_index = i;
    }
    if let Some(c) = o.bench_client_count {
        cfg.bench_client_count = c.max(1);
    }
    if let Some(k) = o.pool_count {
        cfg.pool_count = k.max(1);
    }
    if let Some(layout) = o.pool_layout {
        cfg.pool_layout = layout;
    }
    if let Some(n) = o.worker_host_count {
        cfg.worker_host_count = n.max(1);
    }
    if let Some(topo) = o.storage_topology {
        cfg.storage_topology = Some(topo);
    }
    if let Some(b) = o.tick_batch_limit {
        cfg.tick_batch_limit = b.max(1);
    }
}

fn apply_env_overrides(cfg: &mut BenchRunConfig) {
    if let Ok(v) = std::env::var("CHRONON_BENCH_WORKER_COUNT") {
        if let Ok(w) = v.parse::<u32>() {
            cfg.worker_count = w.max(1);
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_PARTITIONS") {
        if let Ok(p) = v.parse::<u32>() {
            cfg.partition_count = p.max(1);
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_PREFILL") {
        if let Ok(q) = v.parse::<u64>() {
            cfg.prefill_count = q.max(1);
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_CLIENT_INDEX") {
        if let Ok(i) = v.parse::<u32>() {
            cfg.bench_client_index = i;
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_CLIENT_COUNT") {
        if let Ok(c) = v.parse::<u32>() {
            cfg.bench_client_count = c.max(1);
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_POOL_COUNT") {
        if let Ok(k) = v.parse::<u32>() {
            cfg.pool_count = k.max(1);
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_POOL_LAYOUT") {
        cfg.pool_layout = match v.to_ascii_lowercase().as_str() {
            "distinct" => PoolLayout::Distinct,
            _ => PoolLayout::Shared,
        };
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_WORKER_HOSTS") {
        if let Ok(n) = v.parse::<u32>() {
            cfg.worker_host_count = n.max(1);
        }
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_STORAGE_TOPOLOGY") {
        cfg.storage_topology = Some(v);
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_TIER") {
        cfg.tier_tag = Some(v);
    }
    if let Ok(v) = std::env::var("CHRONON_DATA_TIER_PROFILE") {
        cfg.data_tier_profile = Some(v);
    }
    if let Ok(v) = std::env::var("CHRONON_BENCH_TICK_BATCH_LIMIT") {
        if let Ok(b) = v.parse::<u32>() {
            cfg.tick_batch_limit = b.max(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_bench_config, BenchConfigOverrides};
    use crate::config::PoolLayout;

    #[test]
    fn cli_overrides_beat_defaults() {
        let cfg = resolve_bench_config(
            "bm-ch7",
            BenchConfigOverrides {
                worker_count: Some(64),
                prefill_count: Some(5000),
                ..BenchConfigOverrides::default()
            },
        );
        assert_eq!(cfg.worker_count, 64);
        assert_eq!(cfg.prefill_count, 5000);
    }

    #[test]
    fn pool_count_override() {
        let cfg = resolve_bench_config(
            "bm-ch7",
            BenchConfigOverrides {
                pool_count: Some(4),
                pool_layout: Some(PoolLayout::Distinct),
                ..BenchConfigOverrides::default()
            },
        );
        assert_eq!(cfg.pool_count, 4);
        assert_eq!(cfg.pool_layout, PoolLayout::Distinct);
    }
}
