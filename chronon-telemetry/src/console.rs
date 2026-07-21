use super::TelemetrySink;

/// Writes telemetry to stderr (development and bench).
///
/// When a `tracing` subscriber is installed, [`Self::log_event`] also emits a structured
/// `tracing::info` event so host logs and stderr stay aligned.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConsoleSink;

impl TelemetrySink for ConsoleSink {
    fn record_counter(&self, name: &str, labels: &[(&str, &str)], delta: u64) {
        eprintln!("[chronon] counter {name} +{delta} {labels:?}");
    }

    fn record_gauge(&self, name: &str, labels: &[(&str, &str)], value: f64) {
        eprintln!("[chronon] gauge {name} = {value} {labels:?}");
    }

    fn log_event(&self, schema: &str, fields: &[(&str, &str)]) {
        eprintln!("[chronon] event {schema} {fields:?}");
        tracing::info!(target: "chronon_telemetry", schema, ?fields, "telemetry event");
    }
}
