//! BM-CH1 job-count scaling curve.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::{load_reports, scaling_exponent};

#[derive(Debug, Clone, Serialize)]
pub struct JobPoint {
    pub job_count: usize,
    pub query_p95_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct Ch1JobCurve {
    pub hardware: String,
    pub storage: String,
    pub experiment: String,
    pub points: Vec<JobPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling_exponent: Option<f64>,
    pub verdict: String,
}

/// Build CH1 job-count scaling curve from report JSONs.
pub fn ch1_job_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-ch1", storage, hardware)?;
    if reports.is_empty() {
        bail!("no bm-ch1 reports for storage={storage} hardware={hardware}");
    }

    let mut points = Vec::new();
    for report in &reports {
        let jobs = report
            .sweep_dimensions
            .as_ref()
            .and_then(|d| d.job_count)
            .or(report.jobs)
            .unwrap_or(0);
        let p95 = report.query_ms.as_ref().map_or(0.0, |s| s.p95);
        if jobs == 0 {
            continue;
        }
        points.push(JobPoint {
            job_count: jobs,
            query_p95_ms: p95,
        });
    }
    points.sort_by_key(|p| p.job_count);
    dedupe_best_per_job(&mut points);

    let scaling_exp = if points.len() >= 2 {
        let first = &points[0];
        let last = points.last().expect("len >= 2");
        scaling_exponent(
            first.query_p95_ms.max(1e-9),
            last.query_p95_ms.max(1e-9),
            first.job_count as f64,
            last.job_count as f64,
        )
    } else {
        None
    };

    let verdict = if scaling_exp.is_some_and(|e| e < 1.5) {
        "query_sublinear"
    } else {
        "query_superlinear"
    }
    .to_string();

    let curve = Ch1JobCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        experiment: "bm-ch1".to_string(),
        points,
        scaling_exponent: scaling_exp,
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!("scaling-curve-ch1-jobs-{storage}-{hardware}.json"))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}

fn dedupe_best_per_job(points: &mut Vec<JobPoint>) {
    let mut best: std::collections::BTreeMap<usize, JobPoint> = std::collections::BTreeMap::new();
    for p in points.drain(..) {
        best.entry(p.job_count)
            .and_modify(|e| {
                if p.query_p95_ms < e.query_p95_ms || e.query_p95_ms == 0.0 {
                    *e = p.clone();
                }
            })
            .or_insert(p);
    }
    *points = best.into_values().collect();
}

#[cfg(test)]
mod tests {
    use super::{dedupe_best_per_job, JobPoint};

    #[test]
    fn dedupe_keeps_lowest_p95() {
        let mut pts = vec![
            JobPoint {
                job_count: 1000,
                query_p95_ms: 5.0,
            },
            JobPoint {
                job_count: 1000,
                query_p95_ms: 3.0,
            },
        ];
        dedupe_best_per_job(&mut pts);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].query_p95_ms - 3.0).abs() < f64::EPSILON);
    }
}
