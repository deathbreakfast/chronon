# chronon-telemetry

`TelemetrySink` trait and console/no-op adapters — install `NoOpSink` or `ConsoleSink` on `ChrononBuilder`, or implement custom metrics/event sinks.

## Shipped adapters

| Type | Role |
|------|------|
| `NoOpSink` | Default — discards telemetry |
| `ConsoleSink` | stderr logging for dev/bench |

## Documentation

```bash
cargo doc -p chronon-telemetry --no-deps --open
```
