//! BM-CH7 worker-count scaling curve.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::{load_reports, scaling_exponent};

#[derive(Debug, Clone, Serialize)]
pub struct WorkerPoint {
    pub worker_count: u32,
    pub claim_ops_per_sec: f64,
    pub report_experiment: String,
}

#[derive(Debug, Serialize)]
pub struct Ch7WorkerCurve {
    pub hardware: String,
    pub storage: String,
    pub experiment: String,
    pub points: Vec<WorkerPoint>,
    pub peak_claim_ops_per_sec: f64,
    pub peak_worker_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling_exponent: Option<f64>,
    pub verdict: String,
}

/// Build CH7 worker scaling curve from report JSONs.
pub fn ch7_worker_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-ch7", storage, hardware)?;
    if reports.is_empty() {
        bail!("no bm-ch7 reports for storage={storage} hardware={hardware} in {}", reports_dir.display());
    }

    let mut points = Vec::new();
    for report in &reports {
        let Some(w) = report
            .sweep_dimensions
            .as_ref()
            .and_then(|d| d.worker_count)
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
        points.push(WorkerPoint {
            worker_count: w,
            claim_ops_per_sec: rate,
            report_experiment: report.experiment.clone(),
        });
    }
    points.sort_by_key(|p| p.worker_count);
    dedupe_best_per_worker(&mut points);

    let (peak_rate, peak_w) = points
        .iter()
        .map(|p| (p.claim_ops_per_sec, p.worker_count))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0.0, 0));

    let scaling_exp = if points.len() >= 2 {
        let first = &points[0];
        let last = points.last().expect("len >= 2");
        scaling_exponent(
            first.claim_ops_per_sec,
            last.claim_ops_per_sec,
            f64::from(first.worker_count),
            f64::from(last.worker_count),
        )
    } else {
        None
    };

    let verdict = if scaling_exp.is_some_and(|e| e > 0.5) {
        "worker_scaling"
    } else if points.len() <= 1 {
        "single_point"
    } else {
        "worker_saturated"
    }
    .to_string();

    let curve = Ch7WorkerCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        experiment: "bm-ch7".to_string(),
        points,
        peak_claim_ops_per_sec: peak_rate,
        peak_worker_count: peak_w,
        scaling_exponent: scaling_exp,
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!("scaling-curve-ch7-workers-{storage}-{hardware}.json"))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}

fn dedupe_best_per_worker(points: &mut Vec<WorkerPoint>) {
    let mut best: std::collections::BTreeMap<u32, WorkerPoint> = std::collections::BTreeMap::new();
    for p in points.drain(..) {
        best.entry(p.worker_count)
            .and_modify(|e| {
                if p.claim_ops_per_sec > e.claim_ops_per_sec {
                    *e = p.clone();
                }
            })
            .or_insert(p);
    }
    *points = best.into_values().collect();
}

#[cfg(test)]
mod tests {
    use super::{dedupe_best_per_worker, WorkerPoint};

    #[test]
    fn dedupe_keeps_highest_rate() {
        let mut pts = vec![
            WorkerPoint {
                worker_count: 8,
                claim_ops_per_sec: 100.0,
                report_experiment: "bm-ch7".into(),
            },
            WorkerPoint {
                worker_count: 8,
                claim_ops_per_sec: 200.0,
                report_experiment: "bm-ch7".into(),
            },
        ];
        dedupe_best_per_worker(&mut pts);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].claim_ops_per_sec - 200.0).abs() < f64::EPSILON);
    }
}
