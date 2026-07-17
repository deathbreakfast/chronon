//! BM-CH7D worker-fleet scaling curve (D4).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::load_reports;

#[derive(Debug, Clone, Serialize)]
pub struct WorkerFleetPoint {
    pub worker_host_count: u32,
    pub claim_ops_per_sec: f64,
}

#[derive(Debug, Serialize)]
pub struct Ch7dFleetCurve {
    pub hardware: String,
    pub storage: String,
    pub experiment: String,
    pub points: Vec<WorkerFleetPoint>,
    pub peak_claim_ops_per_sec: f64,
    pub peak_worker_host_count: u32,
    pub verdict: String,
}

/// Build BM-CH7D worker-host scaling curve.
pub fn ch7d_fleet_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-ch7d", storage, hardware)?;
    let mut points = Vec::new();

    for report in &reports {
        let Some(wn) = report
            .sweep_dimensions
            .as_ref()
            .and_then(|d| d.worker_host_count)
        else {
            continue;
        };
        let rate = report
            .claim_ops_per_sec
            .as_ref()
            .map_or(0.0, |s| s.max);
        if rate <= 0.0 {
            continue;
        }
        points.push(WorkerFleetPoint {
            worker_host_count: wn,
            claim_ops_per_sec: rate,
        });
    }

    if points.is_empty() {
        bail!("no bm-ch7d reports for storage={storage} hardware={hardware}");
    }

    points.sort_by_key(|p| p.worker_host_count);
    dedupe_best(&mut points);

    let (peak_rate, peak_wn) = points
        .iter()
        .map(|p| (p.claim_ops_per_sec, p.worker_host_count))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0.0, 0));

    let verdict = if points.len() <= 1 {
        "single_point".into()
    } else {
        "worker_fleet_scaling".into()
    };

    let curve = Ch7dFleetCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        experiment: "bm-ch7d".to_string(),
        points,
        peak_claim_ops_per_sec: peak_rate,
        peak_worker_host_count: peak_wn,
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!("scaling-curve-ch7d-fleet-{storage}-{hardware}.json"))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}

fn dedupe_best(points: &mut Vec<WorkerFleetPoint>) {
    let mut best: std::collections::BTreeMap<u32, WorkerFleetPoint> = std::collections::BTreeMap::new();
    for p in points.drain(..) {
        best.entry(p.worker_host_count)
            .and_modify(|e| {
                if p.claim_ops_per_sec > e.claim_ops_per_sec {
                    *e = p.clone();
                }
            })
            .or_insert(p);
    }
    *points = best.into_values().collect();
}
