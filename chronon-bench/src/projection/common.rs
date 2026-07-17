//! Shared helpers for scaling-curve projection.

use std::path::Path;

use anyhow::Result;

use crate::report::BenchReport;

/// Load bench reports from a directory filtered by experiment prefix, storage, and hardware.
pub fn load_reports(
    reports_dir: &Path,
    experiment_prefix: &str,
    storage: &str,
    hardware: &str,
) -> Result<Vec<BenchReport>> {
    let mut reports = Vec::new();
    if !reports_dir.exists() {
        return Ok(reports);
    }

    for entry in std::fs::read_dir(reports_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let fname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if fname.starts_with("scaling-curve-") {
            continue;
        }
        if !fname.starts_with(experiment_prefix) {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let report: BenchReport = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if report.storage != storage || report.hardware != hardware {
            continue;
        }
        reports.push(report);
    }
    reports.sort_by(|a, b| a.recorded_at.cmp(&b.recorded_at));
    Ok(reports)
}

/// Compute scaling exponent from first and last point values (log-log slope).
#[must_use]
pub fn scaling_exponent(first: f64, last: f64, first_knob: f64, last_knob: f64) -> Option<f64> {
    if first <= 0.0 || last <= 0.0 || first_knob <= 0.0 || last_knob <= 0.0 {
        return None;
    }
    if (first_knob - last_knob).abs() < f64::EPSILON {
        return None;
    }
    let ratio = (last / first).log(last_knob / first_knob);
    Some(ratio)
}

#[cfg(test)]
mod tests {
    use super::scaling_exponent;

    #[test]
    fn scaling_exponent_doubles() {
        let exp = scaling_exponent(100.0, 200.0, 1.0, 2.0).unwrap();
        assert!((exp - 1.0).abs() < 0.01);
    }
}
