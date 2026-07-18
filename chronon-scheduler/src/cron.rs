//! Cron expression parsing and next-run calculation.

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use chronon_core::{ChrononError, Result};

/// A parsed cron expression ready for next-run calculations.
///
/// Used when upserting [`chronon_core::Job`] rows with [`chronon_core::ScheduleKind::Cron`]:
/// set `job.cron_expr` (and optional `timezone`); `CoordinatorService::upsert_job` calls
/// [`Self::parse`] and stores `next_run_at`.
///
/// Syntax: standard five-field cron (`minute hour day-of-month month day-of-week`). An optional
/// sixth field enables seconds (`CronExpr::has_seconds`). Timezone names follow `chrono-tz`
/// (e.g. `"America/New_York"`); omit for UTC.
///
/// # Examples
///
/// ```
/// use chronon_scheduler::CronExpr;
///
/// let cron = CronExpr::parse("0 2 * * *", Some("UTC")).unwrap();
/// assert_eq!(cron.expression(), "0 2 * * *");
/// assert!(!cron.has_seconds());
/// assert!(cron.next_from_now().is_some());
///
/// let with_secs = CronExpr::parse("0 0 2 * * *", None).unwrap();
/// assert!(with_secs.has_seconds());
/// ```
#[derive(Debug, Clone)]
pub struct CronExpr {
    expr: String,
    schedule: croner::Cron,
    timezone: Tz,
}

impl CronExpr {
    /// Parse a cron expression with an optional timezone.
    ///
    /// # Examples
    ///
    /// ```
    /// use chronon_scheduler::CronExpr;
    ///
    /// let cron = CronExpr::parse("0 0 * * *", None).unwrap();
    /// assert_eq!(cron.expression(), "0 0 * * *");
    /// assert!(cron.next_from_now().is_some());
    /// ```
    pub fn parse(expr: &str, timezone: Option<&str>) -> Result<Self> {
        let schedule = croner::Cron::new(expr)
            .with_seconds_optional()
            .parse()
            .map_err(|e| ChrononError::InvalidCron(format!("{expr}: {e}")))?;

        let tz: Tz = match timezone {
            Some(tz_str) => tz_str
                .parse()
                .map_err(|_| ChrononError::InvalidTimezone(tz_str.to_string()))?,
            None => Tz::UTC,
        };

        Ok(Self {
            expr: expr.to_string(),
            schedule,
            timezone: tz,
        })
    }

    /// Calculate the next run time after the given datetime.
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let after_tz = after.with_timezone(&self.timezone);
        self.schedule
            .find_next_occurrence(&after_tz, false)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Calculate the next run time from now.
    pub fn next_from_now(&self) -> Option<DateTime<Utc>> {
        self.next_after(Utc::now())
    }

    /// Get the original cron expression.
    pub fn expression(&self) -> &str {
        &self.expr
    }

    /// Get the timezone.
    pub fn timezone(&self) -> &Tz {
        &self.timezone
    }

    /// Check if the expression uses seconds (6 fields).
    pub fn has_seconds(&self) -> bool {
        self.expr.split_whitespace().count() == 6
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    #[test]
    fn parse_valid_cron() {
        assert!(CronExpr::parse("0 0 * * *", None).is_ok());
    }

    #[test]
    fn parse_with_timezone() {
        let cron = CronExpr::parse("0 9 * * *", Some("America/New_York")).unwrap();
        assert_eq!(*cron.timezone(), chrono_tz::America::New_York);
    }

    #[test]
    fn parse_invalid_cron() {
        assert!(CronExpr::parse("invalid", None).is_err());
    }

    #[test]
    fn next_after_daily_noon() {
        let cron = CronExpr::parse("0 12 * * *", None).unwrap();
        let base = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let next = cron.next_after(base).unwrap();
        assert_eq!(next.hour(), 12);
        assert_eq!(next.day(), 1);
    }
}
