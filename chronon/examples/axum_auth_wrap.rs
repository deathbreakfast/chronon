//! Demonstrate [`RequireAdmin`] + [`StaticTokenAdminAuth`] on the Chronon HTTP API.
//!
//! Install a host [`AdminAuth`] verifier, set require-admin-auth, and nest under
//! [`API_PREFIX`] before public bind. Higgs (or equivalent) owns production identity.
//!
//! ```bash
//! cargo run -p uf-chronon --example axum_auth_wrap --features mem,axum
//! ```

use std::sync::Arc;

use axum::body::Body;
use axum::extract::FromRef;
use axum::http::{Request, StatusCode};
use axum::Router;
use chronon::prelude::*;
use chronon_axum::{
    chronon_router, ApiResponse, ChrononState, ScriptResponse, StaticTokenAdminAuth, API_PREFIX,
};
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_executor::{ScriptDescriptor, ScriptRegistry};
use http_body_util::BodyExt;
use tower::ServiceExt;

const DEMO_TOKEN: &str = "demo-chronon-admin-token";

fn noop_script(
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

    let store = Arc::new(InMemorySchedulerStore::new());
    let coordinator = Arc::new(CoordinatorService::new(store));
    let registry = Arc::new({
        let mut r = ScriptRegistry::new();
        r.register(&ScriptDescriptor::new("http_demo", noop_script));
        r
    });

    let chronon = ChrononState::builder(coordinator, registry)
        .admin_auth(Arc::new(StaticTokenAdminAuth::new(DEMO_TOKEN)))
        .require_admin_auth(true)
        .build()
        .map_err(chronon::ChrononError::Internal)?;

    let app = Router::new()
        .nest(API_PREFIX, chronon_router::<AppState>())
        .with_state(AppState { chronon });

    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("{API_PREFIX}/scripts"))
                .body(Body::empty())
                .map_err(|e| chronon::ChrononError::Internal(e.to_string()))?,
        )
        .await
        .map_err(|e| chronon::ChrononError::Internal(e.to_string()))?;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let allowed = app
        .oneshot(
            Request::builder()
                .uri(format!("{API_PREFIX}/scripts"))
                .header("x-chronon-admin-token", DEMO_TOKEN)
                .body(Body::empty())
                .map_err(|e| chronon::ChrononError::Internal(e.to_string()))?,
        )
        .await
        .map_err(|e| chronon::ChrononError::Internal(e.to_string()))?;
    assert_eq!(allowed.status(), StatusCode::OK);
    let body = allowed
        .into_body()
        .collect()
        .await
        .map_err(|e| chronon::ChrononError::Internal(e.to_string()))?
        .to_bytes();
    let parsed: ApiResponse<Vec<ScriptResponse>> = serde_json::from_slice(&body)
        .map_err(|e| chronon::ChrononError::Internal(e.to_string()))?;
    assert!(parsed.success);
    assert_eq!(parsed.data.as_ref().map(std::vec::Vec::len), Some(1));

    eprintln!(
        "Chronon API at {API_PREFIX} — missing token → 401, x-chronon-admin-token → listed 1 script"
    );
    Ok(())
}
