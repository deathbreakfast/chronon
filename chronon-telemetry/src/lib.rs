//! Chronon self-telemetry port.
//!
//! Host-injectable metrics and structured events for the scheduler and executor.
//! Install a [`TelemetrySink`] on `ChrononBuilder` in `chronon-runtime` at boot.
//!
//! # Telemetry vs tracing
//!
//! - **[`TelemetrySink`]** — domain metrics/counters and testable structured events
//!   (`record_counter`, `record_gauge`, `log_event`). Hosts implement sinks for Prometheus,
//!   StatsD, or in-memory capture ([`RecordingSink`]).
//! - **`tracing`** — diagnostic spans and logs in runtime/scheduler/executor. Library crates
//!   emit spans; **hosts** initialize `tracing_subscriber` (never `ChrononBuilder`).
//!
//! # Sinks
//!
//! - [`NoOpSink`] — default discard sink
//! - [`ConsoleSink`] — stderr output for development (also emits `tracing` events when a
//!   subscriber is installed)
//! - [`RecordingSink`] — in-memory capture for test assertions
//!
//! # Metric names
//!
//! Stable counters include `chronon_scheduler_ticks`, `chronon_runs_started`,
//! `chronon_runs_completed`, and `chronon_runs_failed`. Label keys are not validated;
//! keep names stable for dashboard compatibility.

mod console;
mod noop;
mod recording;

pub use console::ConsoleSink;
pub use noop::NoOpSink;
pub use recording::{RecordedCounter, RecordedEvent, RecordedGauge, RecordingSink};

/// Host-injectable telemetry sink for scheduler and executor metrics/events.
///
/// Implement for Prometheus, StatsD, or host logging; install on `ChrononBuilder` at boot.
pub trait TelemetrySink: Send + Sync {
    /// Increment a counter by `delta` with optional string labels.
    fn record_counter(&self, name: &str, labels: &[(&str, &str)], delta: u64);

    /// Set a gauge to an absolute `value` with optional string labels.
    fn record_gauge(&self, name: &str, labels: &[(&str, &str)], value: f64);

    /// Emit a structured event identified by `schema` with string field pairs.
    fn log_event(&self, schema: &str, fields: &[(&str, &str)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_sink_accepts_metrics() {
        let sink = NoOpSink;
        sink.record_counter("chronon_scheduler_ticks", &[("component", "scheduler")], 1);
        sink.record_gauge("chronon_active_runs", &[], 0.0);
        sink.log_event("chronon.run.started", &[("run_id", "abc")]);
    }

    #[test]
    fn console_sink_accepts_metrics() {
        let sink = ConsoleSink;
        sink.record_counter("chronon_runs_started", &[], 1);
    }
}
