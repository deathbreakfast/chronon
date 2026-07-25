# chronon-axum

HTTP API under `/api/chronon/*` — jobs, runs, scripts.

## Production security

Chronon does **not** authenticate these routes. Hosts must wrap `chronon_router` with authn/authz (and ideally rate limits) before exposing it on a network. See [SECURITY.md](../SECURITY.md) and the `axum_auth_wrap` example.

## Mount

```rust
use chronon_axum::{chronon_router, ChrononState, API_PREFIX};

// Nest under API_PREFIX, then apply host middleware around the nest (not shown).
Router::new().nest(API_PREFIX, chronon_router::<AppState>())
```

## Behavior notes

| Topic | Behavior |
|-------|----------|
| Upsert | Matched by `job_name`; updates preserve `job_id` and bump revision |
| List jobs/runs | Default `limit` 100, capped at 1000 |
| Revisions | HTTP response redacts actor/params; store keeps full snapshots |
| `actor_json` | Host responsibility on external APIs; execution uses run snapshot |
