//! Deterministic partition bucketing and env-driven scheduler tuning.

use xxhash_rust::xxh64::xxh64;

/// Stable `xxhash64(job_id) % num_partitions` in `[0, num_partitions)`.
///
/// Used when assigning jobs to coordinator partitions; `num_partitions` is clamped to at
/// least 1.
pub fn partition_index_for_job_id(job_id: &str, num_partitions: u32) -> u32 {
    let n = num_partitions.max(1);
    let h = xxh64(job_id.as_bytes(), 0);
    (h % u64::from(n)) as u32
}

/// Stored on [`Job::partition_hash`](chronon_core::Job::partition_hash).
///
/// Uses [`num_partitions_from_env`] as the modulus at persistence time.
pub fn partition_hash_i64_for_job_id(job_id: &str) -> i64 {
    i64::from(partition_index_for_job_id(job_id, num_partitions_from_env()))
}

/// Default pool when a job has no `pool` set.
pub const DEFAULT_POOL: &str = "general";

/// Resolved pool id for scheduling and worker claims.
///
/// Trims whitespace; empty or missing job pool falls back to [`DEFAULT_POOL`].
pub fn job_execution_pool_id(job: &chronon_core::Job) -> String {
    job.pool
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_POOL.to_string())
}

const DEFAULT_NUM_PARTITIONS: u32 = 64;
const DEFAULT_TICK_INTERVAL_MS: u64 = 250;

/// Reads `CHRONON_NUM_PARTITIONS` (default 64). Must be >= 1.
pub fn num_partitions_from_env() -> u32 {
    std::env::var("CHRONON_NUM_PARTITIONS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(DEFAULT_NUM_PARTITIONS)
}

/// Reads `CHRONON_TICK_INTERVAL_MS` (default 250).
pub fn tick_interval_ms_from_env() -> u64 {
    std::env::var("CHRONON_TICK_INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(DEFAULT_TICK_INTERVAL_MS)
}

/// Reads `CHRONON_TICK_BATCH_LIMIT` (default 500).
pub fn tick_batch_limit_from_env() -> u32 {
    std::env::var("CHRONON_TICK_BATCH_LIMIT")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(500)
}

/// Reads `CHRONON_JOB_CLAIM_LEASE_TTL_S` (default 5).
pub fn job_claim_lease_ttl_secs() -> i64 {
    std::env::var("CHRONON_JOB_CLAIM_LEASE_TTL_S")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(5)
}

/// Reads `CHRONON_PARTITION_LEASE_TTL_S` (default 30).
pub fn partition_lease_ttl_secs() -> i64 {
    std::env::var("CHRONON_PARTITION_LEASE_TTL_S")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(30)
}

/// Reads `CHRONON_PARTITION_LEASE_RENEW_S` (default 5).
pub fn partition_lease_renew_interval_secs() -> u64 {
    std::env::var("CHRONON_PARTITION_LEASE_RENEW_S")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(5)
}

/// Reads `CHRONON_RUN_LEASE_TTL_S` (default 300).
pub fn run_worker_lease_ttl_secs() -> i64 {
    std::env::var("CHRONON_RUN_LEASE_TTL_S")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(300)
}

/// Reads `CHRONON_RUN_LEASE_RENEW_S` (default 1).
pub fn run_worker_lease_renew_secs() -> u64 {
    std::env::var("CHRONON_RUN_LEASE_RENEW_S")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(1)
}

/// Reads `CHRONON_WORKER_POOL` (default `"general"`).
pub fn worker_pool_from_env() -> String {
    std::env::var("CHRONON_WORKER_POOL").unwrap_or_else(|_| "general".to_string())
}

/// Reads `CHRONON_WORKER_CONCURRENCY` (default 4).
pub fn worker_concurrency_from_env() -> usize {
    std::env::var("CHRONON_WORKER_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_stable_for_same_job() {
        let a = partition_index_for_job_id("job-1", 64);
        let b = partition_index_for_job_id("job-1", 64);
        assert_eq!(a, b);
    }

    #[test]
    fn partition_in_range() {
        for i in 0..1000u32 {
            let id = format!("jid-{i}");
            let p = partition_index_for_job_id(&id, 64);
            assert!(p < 64);
        }
    }
}
