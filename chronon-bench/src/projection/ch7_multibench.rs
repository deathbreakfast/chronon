//! BM-CH7 multibench scaling curve — aggregate claims/s vs bench_client_count.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::common::load_reports;
use crate::report::BenchReport;

#[derive(Debug, Clone, Serialize)]
pub struct MultibenchPoint {
    pub bench_client_count: u32,
    pub fleet_claim_ops_per_sec: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fleet_wall_claim_ops_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multibench_efficiency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multibench_wall_efficiency: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Ch7MultibenchCurve {
    pub hardware: String,
    pub storage: String,
    pub experiment: String,
    pub points: Vec<MultibenchPoint>,
    pub peak_fleet_claim_ops_per_sec: f64,
    pub peak_bench_client_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_fleet_wall_claim_ops_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_client_peak: Option<f64>,
    pub verdict: String,
}

fn report_sum_rate(report: &BenchReport) -> f64 {
    report
        .fleet_claim_ops_per_sec
        .or_else(|| report.claim_ops_per_sec.as_ref().map(|s| s.max))
        .unwrap_or(0.0)
}

fn report_wall_rate(report: &BenchReport) -> Option<f64> {
    report.fleet_wall_claim_ops_per_sec.or_else(|| {
        let ops = report.ops?;
        let drain = report.drain_elapsed_secs?;
        if drain <= f64::EPSILON {
            return None;
        }
        Some(ops as f64 / drain)
    })
}

fn report_rate(report: &BenchReport) -> f64 {
    report_wall_rate(report)
        .filter(|r| *r > 0.0)
        .unwrap_or_else(|| report_sum_rate(report))
}

fn update_bc1_peak(peak: &mut Option<(f64, u32)>, rate: f64, workers: u32) {
    if rate <= 0.0 {
        return;
    }
    match *peak {
        None => *peak = Some((rate, workers)),
        Some((_prev, prev_w)) if workers == 1 && prev_w != 1 => {
            *peak = Some((rate, workers));
        }
        Some((prev, prev_w)) if workers == prev_w && rate > prev => {
            *peak = Some((rate, workers));
        }
        Some((prev, prev_w)) if prev_w != 1 && workers != 1 && rate > prev => {
            *peak = Some((rate, workers));
        }
        _ => {}
    }
}

fn record_best(map: &mut HashMap<u32, f64>, bc: u32, rate: f64) {
    if rate > 0.0 {
        map.entry(bc).and_modify(|e| *e = e.max(rate)).or_insert(rate);
    }
}

fn efficiency(primary: f64, bc: u32, per_client: f64) -> Option<f64> {
    if per_client > 0.0 && bc > 0 {
        Some(primary / (f64::from(bc) * per_client))
    } else {
        None
    }
}

fn build_points(
    best_sum: &HashMap<u32, f64>,
    best_wall: &HashMap<u32, f64>,
    per_client: f64,
) -> Vec<MultibenchPoint> {
    let bc_keys: BTreeSet<u32> = best_sum.keys().chain(best_wall.keys()).copied().collect();
    let mut points: Vec<MultibenchPoint> = bc_keys
        .into_iter()
        .map(|bc| {
            let sum_rate = best_sum.get(&bc).copied().unwrap_or(0.0);
            let wall_rate = best_wall.get(&bc).copied();
            let primary = wall_rate.filter(|r| *r > 0.0).unwrap_or(sum_rate);
            MultibenchPoint {
                bench_client_count: bc,
                fleet_claim_ops_per_sec: primary,
                fleet_wall_claim_ops_per_sec: wall_rate,
                multibench_efficiency: efficiency(primary, bc, per_client),
                multibench_wall_efficiency: wall_rate.and_then(|w| efficiency(w, bc, per_client)),
            }
        })
        .collect();
    points.sort_by_key(|p| p.bench_client_count);
    points
}

/// Build multibench scaling curve from aggregate or bc>1 per-client reports.
pub fn ch7_multibench_curve(
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    let reports = load_reports(reports_dir, "bm-ch7", storage, hardware)?;
    let mut best_sum: HashMap<u32, f64> = HashMap::new();
    let mut best_wall: HashMap<u32, f64> = HashMap::new();
    let mut bc1_peak: Option<(f64, u32)> = None;

    for report in &reports {
        let Some(dims) = report.sweep_dimensions.as_ref() else {
            continue;
        };
        let bc = dims.bench_client_count.unwrap_or(1);
        let workers = dims.worker_count.unwrap_or(1);
        if bc <= 1 && !report.aggregate.unwrap_or(false) {
            update_bc1_peak(&mut bc1_peak, report_rate(report), workers);
            continue;
        }
        if bc <= 1 || !report.aggregate.unwrap_or(false) {
            continue;
        }
        let baseline_w = bc1_peak.map_or(1, |(_, bw)| bw);
        if workers != baseline_w {
            continue;
        }
        record_best(&mut best_sum, bc, report_sum_rate(report));
        if let Some(wall) = report_wall_rate(report) {
            record_best(&mut best_wall, bc, wall);
        }
    }

    if best_sum.is_empty() && best_wall.is_empty() {
        bail!("no multibench bm-ch7 reports for storage={storage} hardware={hardware}");
    }

    let per_client = bc1_peak.map_or_else(
        || {
            best_wall
                .get(&1)
                .copied()
                .or_else(|| best_sum.get(&1).copied())
                .or_else(|| best_wall.values().copied().reduce(f64::max))
                .or_else(|| best_sum.values().copied().reduce(f64::max))
                .unwrap_or(0.0)
        },
        |(r, _)| r,
    );

    let points = build_points(&best_sum, &best_wall, per_client);
    let (peak_rate, peak_bc) = points
        .iter()
        .map(|p| (p.fleet_claim_ops_per_sec, p.bench_client_count))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0.0, 0));

    let peak_wall = points
        .iter()
        .filter_map(|p| p.fleet_wall_claim_ops_per_sec)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let wall_eff_at_max_bc = points
        .last()
        .and_then(|p| p.multibench_wall_efficiency.or(p.multibench_efficiency))
        .unwrap_or(0.0);

    let verdict = if points.len() <= 1 {
        "single_point".into()
    } else if wall_eff_at_max_bc >= 0.7 {
        "multibench_scaling".into()
    } else {
        "embed_sublinear".into()
    };

    let curve = Ch7MultibenchCurve {
        hardware: hardware.to_string(),
        storage: storage.to_string(),
        experiment: "bm-ch7".to_string(),
        points,
        peak_fleet_claim_ops_per_sec: peak_rate,
        peak_bench_client_count: peak_bc,
        peak_fleet_wall_claim_ops_per_sec: peak_wall,
        per_client_peak: Some(per_client),
        verdict,
    };

    let out_path = out.unwrap_or_else(|| {
        reports_dir.join(format!(
            "scaling-curve-ch7-multibench-{storage}-{hardware}.json"
        ))
    });
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&curve)?)?;
    println!("wrote {}", out_path.display());
    Ok(())
}
