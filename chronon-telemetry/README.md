# chronon-telemetry

`TelemetrySink` trait and console/no-op adapters.

## Audience

| Reader | Use this crate for |
|--------|-------------------|
| **Integrators** | Installing `NoOpSink` or `ConsoleSink` on `ChrononBuilder` |
| **Adapter authors** | Implementing custom metrics/event sinks |

## Shipped adapters

| Type | Role |
|------|------|
| `NoOpSink` | Default — discards telemetry |
| `ConsoleSink` | stderr logging for dev/bench |

## Documentation

```bash
cargo doc -p chronon-telemetry --no-deps --open
```
