# chronon-runtime

`Chronon`, `ChrononBuilder`, `CoordinatorService`, and runtime loop assembly.

## Audience

| Reader | Use this crate for |
|--------|-------------------|
| **Integrators** | `.embedded()`, `.coordinator_only()`, `.worker()`, `.remote_coordinator()` |
| **Maintainers** | Scheduler + executor wiring, event persistence |

## Deployment shapes (not enums)

Use builder methods — assembly is explicit, not a global deployment flag:

| Method | Shape |
|--------|-------|
| `.embedded()` | Tick + execute in one process |
| `.coordinator_only()` | Tick + enqueue only |
| `.worker(pool_id)` | Claim + execute only |
| `.remote_coordinator(url)` | HTTP client shell |

## Documentation

```bash
cargo doc -p chronon-runtime --no-deps --open
```
