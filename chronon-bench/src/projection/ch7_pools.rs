//! BM-CH7 pool-count scaling curve (D1).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::{load_reports, scaling_exponent};

#[derive(Debug, Clone, Serialize)]
pub struct PoolPoint {
    pub pool_count: u32,
    pub claim_ops_per_sec: f64,
}

#[derive(Debug, Serialize)]
pub struct Ch7PoolCurve {
    pub hardware: String,
    pub storage: String,
    pub experiment: String,
    pub points: Vec<PoolPoint>,
    pub peak_claim_ops_per_sec: f64,
    pub peak_pool_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling_exponent: Option<f64>,
    pub verdict: String,
}

/// Build CH7 pool scaling curve from report JSONs.
pub fn ch7_pool_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-ch7", storage, hardware)?;
    let mut points = Vec::new();

    for report in &reports {
        let Some(k) = report.sweep_dimensions.as_ref().and_then(|d| d.pool_count) else {
            continue;
        };
        let rate = report.claim_ops_per_sec.as_ref().map_or(0.0, |s| s.max);
        if rate <= 0.0 {
            continue;
        }
        points.push(PoolPoint {
            pool_count: k,
            claim_ops_per_sec: rate,
        });
    }

    if points.is_empty() {
        bail!("no pool-dimension bm-ch7 reports for storage={storage} hardware={hardware}");
    }

    points.sort_by_key(|p| p.pool_count);
    dedupe_best_per_pool(&mut points);

    let (peak_rate, peak_k) = points
        .iter()
        .map(|p| (p.claim_ops_per_sec, p.pool_count))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0.0, 0));

    let scaling_exp = if points.len() >= 2 {
        let first = &points[0];
        let last = points.last().expect("len >= 2");
        scaling_exponent(
            first.claim_ops_per_sec,
            last.claim_ops_per_sec,
            f64::from(first.pool_count),
            f64::from(last.pool_count),
        )
    } else {
        None
    };

    let verdict = if scaling_exp.is_some_and(|e| e > 0.3) {
        "pool_scaling".to_string()
    } else {
        "pool_saturated".to_string()
    };

    let curve = Ch7PoolCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        experiment: "bm-ch7".to_string(),
        points,
        peak_claim_ops_per_sec: peak_rate,
        peak_pool_count: peak_k,
        scaling_exponent: scaling_exp,
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!("scaling-curve-ch7-pools-{storage}-{hardware}.json"))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}

fn dedupe_best_per_pool(points: &mut Vec<PoolPoint>) {
    let mut best: std::collections::BTreeMap<u32, PoolPoint> = std::collections::BTreeMap::new();
    for p in points.drain(..) {
        best.entry(p.pool_count)
            .and_modify(|e| {
                if p.claim_ops_per_sec > e.claim_ops_per_sec {
                    *e = p.clone();
                }
            })
            .or_insert(p);
    }
    *points = best.into_values().collect();
}
