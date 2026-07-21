//! BM-CHL sustained due-jobs-per-tick ladder curve.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::load_reports;

#[derive(Debug, Clone, Serialize)]
pub struct SustainPoint {
    pub experiment: String,
    pub due_jobs_per_tick: usize,
    pub tick_p99_ms: f64,
    pub error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct ChlSustainCurve {
    pub hardware: String,
    pub storage: String,
    pub points: Vec<SustainPoint>,
    pub verdict: String,
}

/// Build CHL sustain ladder from report JSONs.
pub fn chl_sustain_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-chl", storage, hardware)?;
    if reports.is_empty() {
        bail!("no bm-chl reports for storage={storage} hardware={hardware}");
    }

    let mut points = Vec::new();
    for report in &reports {
        let due = report
            .sweep_dimensions
            .as_ref()
            .and_then(|d| d.job_count)
            .or(report.jobs)
            .unwrap_or(0);
        let p99 = report.tick_ms.as_ref().map_or(0.0, |s| s.p99);
        let err = report.error_rate.unwrap_or(0.0);
        if due == 0 {
            continue;
        }
        points.push(SustainPoint {
            experiment: report.experiment.clone(),
            due_jobs_per_tick: due,
            tick_p99_ms: p99,
            error_rate: err,
        });
    }
    points.sort_by_key(|p| p.due_jobs_per_tick);

    let verdict = if points.iter().all(|p| p.error_rate < 0.001) {
        "sustain_pass"
    } else {
        "sustain_fail"
    }
    .to_string();

    let curve = ChlSustainCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        points,
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!(
            "scaling-curve-chl-sustain-{storage}-{hardware}.json"
        ))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}
