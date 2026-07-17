//! Lightweight percentile stats for Chronon bench reports.

use serde::{Deserialize, Serialize};

/// Summary statistics for a sample set (min, percentiles, max, count).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MetricStats {
    /// Minimum sample value.
    pub min: f64,
    /// 50th percentile.
    pub p50: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Maximum sample value.
    pub max: f64,
    /// Number of samples summarized.
    pub count: usize,
}

impl MetricStats {
    /// Zeroed stats for an empty sample set.
    pub fn empty() -> Self {
        Self {
            min: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
            max: 0.0,
            count: 0,
        }
    }

    /// Sort `samples` in place and compute percentile summary.
    pub fn summarize(mut samples: Vec<f64>) -> Self {
        if samples.is_empty() {
            return Self::empty();
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = samples.len();
        Self {
            min: samples[0],
            p50: percentile(&samples, 0.50),
            p95: percentile(&samples, 0.95),
            p99: percentile(&samples, 0.99),
            max: samples[count - 1],
            count,
        }
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 * p).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_single_sample() {
        let s = MetricStats::summarize(vec![1.0]);
        assert_eq!(s.count, 1);
        assert!((s.p50 - 1.0).abs() < f64::EPSILON);
    }
}
