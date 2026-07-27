//! Postgres + Redis storage, Axum admin auth, and the remote HTTP client topology combined —
//! **Production-shaped remote host** (auth-gated coordinator).
//!
//! Requires PostgreSQL and Redis. Set `CHRONON_POSTGRES_URL` and ensure Redis is reachable
//! (default `redis://127.0.0.1:6379`, or `CHRONON_REDIS_URL`). Local infra:
//!
//! ```bash
//! docker run -d --name chronon-pg -e POSTGRES_USER=chronon -e POSTGRES_PASSWORD=chronon \
//!   -e POSTGRES_DB=chronon -p 5432:5432 postgres:16
//! docker run -d --name chronon-redis -p 6379:6379 redis:7
//!
//! export CHRONON_POSTGRES_URL=postgres://chronon:chronon@localhost:5432/chronon
//! cargo run -p uf-chronon --example authenticated_remote_postgres_redis --features postgres,redis,axum
//! ```
//!
//! Combines three Wave-1 pieces:
//!
//! 1. **Storage** — Postgres + Redis composite store (`postgres_redis_boot` / `coordinator_daemon`).
//! 2. **Auth** — [`StaticTokenAdminAuth`] + `require_admin_auth(true)` gating every route
//!    (`axum_auth_wrap`); Chronon does not authenticate `/api/chronon/*` on its own — see
//!    repository `SECURITY.md`.
//! 3. **Remote client** — [`RemoteCoordinatorClient`] against the mounted API (`remote_http_client`).
//!
//! [`RemoteCoordinatorClient`] does not attach custom headers today, so an unauthenticated call
//! through it is denied (proved in-process below). Hosts that need authenticated remote access
//! send the admin header themselves — either with `reqwest` directly (also proved below) or via
//! `curl`:
//!
//! ```bash
//! # Denied — no admin token
//! curl -i http://127.0.0.1:PORT/api/chronon/jobs
//!
//! # Allowed — admin token attached
//! curl -i http://127.0.0.1:PORT/api/chronon/jobs -H 'x-chronon-admin-token: <token>'
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::FromRef;
use axum::Router;
use chronon::prelude::*;
use chronon_axum::{chronon_router, ChrononState, StaticTokenAdminAuth, API_PREFIX};
use chronon_backend_postgres::{postgres_test_url, PostgresSchedulerStore};
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};
use chronon_executor::{ScriptDescriptor, ScriptRegistry};
use tokio::net::TcpListener;

const DEMO_TOKEN: &str = "demo-chronon-admin-token";

fn remote_pg_redis_demo_script(
    _ctx: Box<dyn ScriptContext>,
    _params: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = chronon::Result<()>> + Send>> {
    Box::pin(async { Ok(()) })
}

#[derive(Clone)]
struct AppState {
    chronon: ChrononState,
}

impl FromRef<AppState> for ChrononState {
    fn from_ref(state: &AppState) -> Self {
        state.chronon.clone()
    }
}

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let pg_url = postgres_test_url();
    let redis_url =
        std::env::var("CHRONON_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let sql: Arc<dyn SchedulerStore> = Arc::new(PostgresSchedulerStore::connect(&pg_url).await?);
    let redis = RedisQueueLayer::connect(&redis_url, None).await?;
    let store: Arc<dyn SchedulerStore> = Arc::new(PostgresRedisSchedulerStore::new(sql, redis));

    let coordinator = Arc::new(CoordinatorService::new(store));
    let registry = Arc::new({
        let mut r = ScriptRegistry::new();
        r.register(&ScriptDescriptor::new(
            "remote_pg_redis_demo",
            remote_pg_redis_demo_script,
        ));
        r
    });

    let chronon_state = ChrononState::builder(coordinator, registry)
        .admin_auth(Arc::new(StaticTokenAdminAuth::new(DEMO_TOKEN)))
        .require_admin_auth(true)
        .build()?;

    let app = Router::new()
        .nest(API_PREFIX, chronon_router::<AppState>())
        .with_state(AppState {
            chronon: chronon_state,
        });

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .map_err(|e| ChrononError::Internal(format!("bind: {e}")))?;
    let addr = listener
        .local_addr()
        .map_err(|e| ChrononError::Internal(format!("local_addr: {e}")))?;
    let base = format!("http://{addr}");

    let server = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("authenticated demo API host stopped: {e}");
        }
    });
    // Brief yield so the accept loop is listening before the first HTTP call.
    tokio::task::yield_now().await;

    // Deny: RemoteCoordinatorClient sends no admin header, so the auth-gated route rejects it.
    let unauthenticated = RemoteCoordinatorClient::new(base.clone());
    let denied = unauthenticated.list_jobs().await;
    assert!(
        denied.is_err(),
        "expected unauthenticated remote client call to be denied"
    );

    // Allow: attach the admin token header directly. RemoteCoordinatorClient has no header
    // hook today, so hosts needing authenticated remote access send the header themselves
    // (as here, or via the `curl` runbook in this file's module docs).
    let http = reqwest::Client::new();
    let resp = http
        .get(format!("{base}{API_PREFIX}/jobs"))
        .header("x-chronon-admin-token", DEMO_TOKEN)
        .send()
        .await
        .map_err(|e| ChrononError::Internal(format!("authenticated GET /jobs: {e}")))?;
    assert!(
        resp.status().is_success(),
        "expected 200 with a valid admin token, got {}",
        resp.status()
    );

    eprintln!(
        "authenticated_remote_postgres_redis: {pg_url} + {redis_url} — missing token denied, x-chronon-admin-token allowed"
    );
    server.abort();
    Ok(())
}
