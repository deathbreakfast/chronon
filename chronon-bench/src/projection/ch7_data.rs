//! BM-CH7 data-tier topology scaling curve (D2).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::load_reports;

#[derive(Debug, Clone, Serialize)]
pub struct DataTopoPoint {
    pub storage_topology: String,
    pub claim_ops_per_sec: f64,
}

#[derive(Debug, Serialize)]
pub struct Ch7DataCurve {
    pub hardware: String,
    pub storage: String,
    pub experiment: String,
    pub points: Vec<DataTopoPoint>,
    pub peak_claim_ops_per_sec: f64,
    pub peak_storage_topology: String,
    pub verdict: String,
}

/// Build CH7 data-tier curve from reports tagged with `storage_topology`.
pub fn ch7_data_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-ch7", storage, hardware)?;
    let mut points = Vec::new();

    for report in &reports {
        let topo = report.storage_topology.clone().or_else(|| {
            report
                .sweep_dimensions
                .as_ref()
                .and_then(|d| d.storage_topology.clone())
        });
        let Some(topo) = topo else {
            continue;
        };
        let rate = report.claim_ops_per_sec.as_ref().map_or(0.0, |s| s.max);
        if rate <= 0.0 {
            continue;
        }
        points.push(DataTopoPoint {
            storage_topology: topo,
            claim_ops_per_sec: rate,
        });
    }

    if points.is_empty() {
        bail!("no storage_topology bm-ch7 reports for storage={storage} hardware={hardware}");
    }

    let (peak_rate, peak_topo) = points
        .iter()
        .map(|p| (p.claim_ops_per_sec, p.storage_topology.clone()))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0.0, String::new()));

    let verdict = if points.len() <= 1 {
        "single_topology".into()
    } else {
        "data_tier_compared".into()
    };

    let curve = Ch7DataCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        experiment: "bm-ch7".to_string(),
        points,
        peak_claim_ops_per_sec: peak_rate,
        peak_storage_topology: peak_topo,
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!("scaling-curve-ch7-data-{storage}-{hardware}.json"))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}
