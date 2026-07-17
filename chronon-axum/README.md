# chronon-axum

HTTP API under `/api/chronon/*` — jobs, runs, scripts.

## Mount

```rust
use chronon_axum::{chronon_router, ChrononState, API_PREFIX};

Router::new().nest(API_PREFIX, chronon_router::<AppState>())
```
