use super::TelemetrySink;

/// Discards all telemetry (default for tests and minimal hosts).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpSink;

impl TelemetrySink for NoOpSink {
    fn record_counter(&self, _name: &str, _labels: &[(&str, &str)], _delta: u64) {}

    fn record_gauge(&self, _name: &str, _labels: &[(&str, &str)], _value: f64) {}

    fn log_event(&self, _schema: &str, _fields: &[(&str, &str)]) {}
}
