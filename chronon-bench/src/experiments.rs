//! Experiment registry and default operation counts.

use anyhow::{bail, Result};

/// Matrix slice names for batch `matrix` subcommand runs.
pub const MATRIX_SLICES: &[&str] = &[
    "adapter-floor",
    "durable-floor",
    "claim-capacity",
    "scheduler-sustain",
    "execution-path",
    "resilience",
    "telemetry-tax",
    "cost-tier",
];

/// All registered benchmark experiment ids (BM-CH* and BM-CHL*).
pub const ALL_EXPERIMENT_IDS: &[&str] = &[
    "bm-ch0", "bm-ch1", "bm-ch2", "bm-ch3", "bm-ch4", "bm-ch5", "bm-ch6", "bm-ch7", "bm-ch7d",
    "bm-chl0", "bm-chl1", "bm-chl2", "bm-chl3",
];

/// Resolved plan for one benchmark run (id plus default iteration counts).
pub struct ExperimentPlan {
    /// Experiment slug (for example `"bm-ch0"`).
    pub id: String,
    /// Default measured operation count when `--ops` is omitted.
    pub default_ops: usize,
    /// Default seeded job count for load experiments when `--jobs` is omitted.
    pub default_jobs: Option<usize>,
}

/// Resolve an experiment id and optional CLI overrides into a runnable plan.
pub fn resolve_experiment(
    id: &str,
    ops: Option<usize>,
    jobs: Option<usize>,
) -> Result<ExperimentPlan> {
    let plan = match id {
        "bm-ch0" => ExperimentPlan {
            id: id.into(),
            default_ops: 1000,
            default_jobs: None,
        },
        "bm-ch1" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(100),
            default_jobs: Some(jobs.unwrap_or(1000)),
        },
        "bm-ch2" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(10_000),
            default_jobs: None,
        },
        "bm-ch3" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(200),
            default_jobs: None,
        },
        "bm-ch4" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(10),
            default_jobs: None,
        },
        "bm-ch5" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(100),
            default_jobs: None,
        },
        "bm-ch6" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(50),
            default_jobs: None,
        },
        "bm-ch7" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(32),
            default_jobs: Some(jobs.unwrap_or(10_000)),
        },
        "bm-ch7d" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(32),
            default_jobs: Some(jobs.unwrap_or(100_000)),
        },
        "bm-chl0" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(500),
            default_jobs: Some(10),
        },
        "bm-chl1" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(500),
            default_jobs: Some(100),
        },
        "bm-chl2" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(200),
            default_jobs: Some(1000),
        },
        "bm-chl3" => ExperimentPlan {
            id: id.into(),
            default_ops: ops.unwrap_or(50),
            default_jobs: Some(10_000),
        },
        other => bail!("unknown experiment id: {other}"),
    };
    Ok(ExperimentPlan {
        default_ops: ops.unwrap_or(plan.default_ops),
        ..plan
    })
}

/// Resolve experiment ids for a named matrix slice.
pub fn subset_experiments(slice: &str) -> Result<Vec<&'static str>> {
    let ids: Vec<&str> = match slice {
        "adapter-floor" => vec!["bm-ch0", "bm-ch1", "bm-ch2"],
        "durable-floor" => vec!["bm-ch0", "bm-ch1"],
        "claim-capacity" => vec!["bm-ch7", "bm-ch7d"],
        "scheduler-sustain" => vec!["bm-chl0", "bm-chl1", "bm-chl2", "bm-chl3"],
        "execution-path" => vec!["bm-ch5", "bm-ch6"],
        "resilience" => vec!["bm-ch3", "bm-ch4"],
        "telemetry-tax" => vec!["bm-ch0"],
        "cost-tier" => vec!["bm-chl1"],
        other => bail!("unknown slice {other}; use {}", MATRIX_SLICES.join("|")),
    };
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::subset_experiments;

    #[test]
    fn durable_floor_has_ch0_and_ch1() {
        let ids = subset_experiments("durable-floor").unwrap();
        assert_eq!(ids, vec!["bm-ch0", "bm-ch1"]);
    }

    #[test]
    fn unknown_slice_errors() {
        assert!(subset_experiments("nope").is_err());
    }
}
