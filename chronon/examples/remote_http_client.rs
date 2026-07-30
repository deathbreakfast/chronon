//! Schedule jobs over HTTP with [`RemoteCoordinatorClient`] — no local Chronon loops.
//!
//! Starts a short-lived API host (mem store + nested [`chronon_router`]), then upserts a
//! manual job and calls `run_now` through the remote client.
//!
//! **Production:** Chronon does not authenticate these routes. Wrap the nested router with
//! host auth (see `axum_auth_wrap` and repository `SECURITY.md`) before exposing it.
//!
//! ```bash
//! cargo run -p uf-chronon --example remote_http_client --features mem,axum
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::FromRef;
use axum::Router;
use chronon::prelude::*;
use chronon_axum::{chronon_router, ChrononState, API_PREFIX};
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_executor::{ScriptDescriptor, ScriptRegistry};
use tokio::net::TcpListener;

fn remote_demo_script(
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

    let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
    let coordinator = Arc::new(CoordinatorService::new(store));
    let registry = Arc::new({
        let mut r = ScriptRegistry::new();
        r.register(&ScriptDescriptor::new("remote_demo", remote_demo_script));
        r
    });

    let app = Router::new()
        .nest(API_PREFIX, chronon_router::<AppState>())
        .with_state(AppState {
            chronon: ChrononState::new(coordinator, registry),
        });

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .map_err(|e| chronon::ChrononError::Internal(format!("bind: {e}")))?;
    let addr = listener
        .local_addr()
        .map_err(|e| chronon::ChrononError::Internal(format!("local_addr: {e}")))?;
    let base = format!("http://{addr}");

    let server = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("demo API host stopped: {e}");
        }
    });

    let client = RemoteCoordinatorClient::new(base);
    // Brief yield so the accept loop is listening before the first HTTP call.
    tokio::task::yield_now().await;

    let job = JobBuilder::new(&ScriptHandle::<()>::new("remote_demo"))
        .name("remote-demo-job")
        .manual()
        .build()?;
    client.upsert_job(job).await?;
    // Upsert-by-name may assign the server job_id; resolve it before run_now.
    let jobs = client.list_jobs().await?;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_name, "remote-demo-job");
    let run_id = client.run_now(&jobs[0].job_id).await?;

    eprintln!("RemoteCoordinatorClient upsert + run_now ok — run_id={run_id}");
    server.abort();
    Ok(())
}
