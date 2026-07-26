use super::TelemetrySink;

/// Writes telemetry via `tracing` (development and bench).
///
/// Prefer installing a `tracing` subscriber in the host. Events use target
/// `chronon_telemetry` so they are filterable without raw stderr prints.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConsoleSink;

impl TelemetrySink for ConsoleSink {
    fn record_counter(&self, name: &str, labels: &[(&str, &str)], delta: u64) {
        tracing::info!(
            target: "chronon_telemetry",
            name,
            delta,
            ?labels,
            "counter"
        );
    }

    fn record_gauge(&self, name: &str, labels: &[(&str, &str)], value: f64) {
        tracing::info!(
            target: "chronon_telemetry",
            name,
            value,
            ?labels,
            "gauge"
        );
    }

    fn log_event(&self, schema: &str, fields: &[(&str, &str)]) {
        tracing::info!(
            target: "chronon_telemetry",
            schema,
            ?fields,
            "telemetry event"
        );
    }
}
