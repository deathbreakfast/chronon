//! Optional verdict tags for benchmark reports.
//!
//! Maps primary metrics to stable taxonomy strings (`worker_scaling`, `cron_faster`, …).

use crate::report::BenchReport;

/// Assign a verdict tag from experiment id and primary metrics.
#[must_use]
pub fn evaluate_verdict(report: &BenchReport) -> Option<String> {
    if report.error_rate.is_some_and(|r| r >= 0.001) {
        return Some("fail_error_rate".into());
    }

    match report.experiment.as_str() {
        "bm-ch7" => {
            if report
                .drain_elapsed_secs
                .is_some_and(|s| s < 10.0)
            {
                return Some("insufficient_sample".into());
            }
            report
                .claim_ops_per_sec
                .as_ref()
                .map(|s| if s.max > 0.0 { "worker_scaling" } else { "no_claims" })
                .map(str::to_string)
        }
        "bm-ch7d" => report
            .claim_ops_per_sec
            .as_ref()
            .map(|s| if s.max > 0.0 { "worker_fleet_scaling" } else { "no_claims" })
            .map(str::to_string),
        id if id.starts_with("bm-chl") => Some("sustain_pass".into()),
        "bm-ch2" => {
            let chronon = report.cron_evals_per_sec.unwrap_or(0.0);
            let baseline = report.cron_baseline_evals_per_sec.unwrap_or(0.0);
            if chronon >= baseline {
                Some("cron_faster".into())
            } else {
                Some("cron_slower".into())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate_verdict;
    use crate::report::BenchReport;
    use chronon_testkit::MatrixSpec;

    #[test]
    fn ch2_verdict_when_faster_than_croner() {
        let mut report = BenchReport::base("bm-ch2", &MatrixSpec::default());
        report.cron_evals_per_sec = Some(200.0);
        report.cron_baseline_evals_per_sec = Some(100.0);
        assert_eq!(evaluate_verdict(&report).as_deref(), Some("cron_faster"));
    }
}
