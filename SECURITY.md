# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

Please report security issues privately to the repository maintainers via GitHub Security Advisories on [unified-field-dev/chronon](https://github.com/unified-field-dev/chronon/security/advisories/new).

Do not open public issues for undisclosed vulnerabilities.

## Production trust boundaries

Chronon is a **library scheduler**, not an authenticated product server. Hosts (for example Higgs) own identity, authorization, and network exposure.

### HTTP control plane (`chronon-axum`)

[`chronon_router`](https://docs.rs/chronon-axum) mounts job/run/script routes under `/api/chronon/*` with **no built-in authentication, authorization, or rate limiting**.

**Before production:** wrap the nested router with host middleware (Bearer/mTLS/session) and/or place it behind an authenticated reverse proxy. Never bind an unauthenticated Chronon API to a public interface.

Axum’s default body limit applies unless the host overrides [`DefaultBodyLimit`](https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html). Chronon does not add its own body-size middleware — set host limits appropriate for your deployment.

See the `axum_auth_wrap` example (`cargo run -p uf-chronon --example axum_auth_wrap --features mem,axum`) for a Tower middleware pattern hosts can copy.

### Identity (`actor_json` / `ContextFactory`)

- At enqueue, Chronon copies job `actor_json` onto the run row.
- At execute, workers and `Executor::spawn_run` rebuild context from the **run snapshot**, not the live job row (queued identity cannot be elevated by a later job update).
- On **external** HTTP APIs, hosts should ignore or replace client-supplied `actor_json` with server-derived identity.
- Implement [`ContextFactory`](https://docs.rs/chronon-core) to **fail closed** on untrusted or incomplete payloads (`ChrononError::Identity`).

### Persistence credentials

Any process that can open the Postgres / SQLite / Redis URLs used by Chronon can enqueue, claim, and observe runs for that deployment (Mode 2 split: coordinator and workers share the store). Treat connection strings as high-privilege secrets. Prefer TLS and authenticated Redis; use a unique Redis `key_prefix` per deployment.

Isolated Postgres schemas (`connect_postgres_isolated` / `CHRONON_POSTGRES_SCHEMA`) accept only allowlisted identifiers (`^[A-Za-z_][A-Za-z0-9_]*$`).

### Script allowlist

Only scripts registered in the host `ScriptRegistry` can run. Treat registration as the execution allowlist.

### HTTP revisions

`GET /jobs/{id}/revisions` redacts `changed_by_actor_json` and strips `actor_json` / `params_json` from `snapshot_json`. Full snapshots remain in the store for host admin tools.

### Residual risk

Scripts run with the process privileges of the worker. Chronon does not provide multi-tenant tenancy IDs or built-in audit logging — partition deployments and add host controls as needed.
