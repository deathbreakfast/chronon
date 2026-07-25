//! Demonstrate wrapping [`chronon_router`] with host authentication middleware.
//!
//! Chronon does not ship identity. Hosts (for example Higgs) must authenticate before
//! exposing `/api/chronon/*`. This example uses a demo Bearer token as a Tower layer pattern.
//!
//! ```bash
//! cargo run -p uf-chronon --example axum_auth_wrap --features mem,axum
//! ```

use std::sync::Arc;

use axum::body::Body;
use axum::extract::FromRef;
use axum::http::{header, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use chronon::prelude::*;
use chronon_axum::{chronon_router, ApiResponse, ChrononState, ScriptResponse, API_PREFIX};
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

async fn require_bearer(req: Request<Body>, next: Next) -> Response {
    let authorized = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {DEMO_TOKEN}"));
    if authorized {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
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

    let chronon_routes = chronon_router::<AppState>();
    let app = Router::new()
        .nest(API_PREFIX, chronon_routes)
        .layer(middleware::from_fn(require_bearer))
        .with_state(AppState {
            chronon: ChrononState::new(coordinator, registry),
        });

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
                .header(header::AUTHORIZATION, format!("Bearer {DEMO_TOKEN}"))
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
    assert_eq!(parsed.data.as_ref().map(|d| d.len()), Some(1));

    eprintln!(
        "Chronon API at {API_PREFIX} — unauthenticated → 401, Bearer demo token → listed 1 script"
    );
    Ok(())
}
