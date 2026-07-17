//! Axum HTTP API for Chronon (`/api/chronon/*`).
//!
//! Mounts job, run, and script routes on a host Axum server. Handlers delegate to
//! [`chronon_runtime::CoordinatorService`] and read script metadata from
//! [`chronon_executor::ScriptRegistry`].
//!
//! # Routes
//!
//! - `GET/POST /jobs/*` — list, upsert, pause, resume, run now
//! - `GET /runs/*` — list and fetch runs
//! - `GET /scripts` — list registered scripts
//!
//! All responses use the [`ApiResponse`] envelope (`success`, `data`, `error`).
//! [`UpsertJobRequest::script_name`] must exist in the registry or upsert returns 400.

mod dto;
mod handlers;
mod handlers_common;
mod state;

use axum::{
    extract::FromRef,
    routing::{get, post},
    Router,
};

pub use dto::{
    JobActionRequest, JobResponse, ListJobsQuery, ListRunsQuery, RunResponse, ScheduleKindDto,
    ScriptResponse, UpsertJobRequest,
};
pub use handlers_common::ApiResponse;
pub use state::ChrononState;

/// API mount prefix for host routers (e.g. `nest(API_PREFIX, chronon_router())`).
pub const API_PREFIX: &str = "/api/chronon";

/// Create the Chronon API router with job, run, and script routes.
///
/// Host state `S` must implement [`FromRef<S>`] for [`ChrononState`].
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use axum::body::Body;
/// use axum::http::{Request, StatusCode};
/// use axum::extract::FromRef;
/// use chronon_axum::{chronon_router, ChrononState};
/// use chronon_backend_mem::InMemorySchedulerStore;
/// use chronon_core::{Result, ScriptContext};
/// use chronon_executor::{ScriptDescriptor, ScriptRegistry};
/// use chronon_runtime::CoordinatorService;
/// use http_body_util::BodyExt;
/// use tower::ServiceExt;
///
/// fn noop(
///     _ctx: Box<dyn ScriptContext>,
///     _params: serde_json::Value,
/// ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
///     Box::pin(async { Ok(()) })
/// }
///
/// #[derive(Clone)]
/// struct AppState {
///     chronon: ChrononState,
/// }
///
/// impl FromRef<AppState> for ChrononState {
///     fn from_ref(state: &AppState) -> Self {
///         state.chronon.clone()
///     }
/// }
///
/// # #[tokio::main]
/// # async fn main() {
/// let store = Arc::new(InMemorySchedulerStore::new());
/// let coordinator = Arc::new(CoordinatorService::new(store));
/// let registry = Arc::new({
///     let mut r = ScriptRegistry::new();
///     r.register(ScriptDescriptor::new("demo", noop));
///     r
/// });
/// let app = chronon_router::<AppState>().with_state(AppState {
///     chronon: ChrononState::new(coordinator, registry),
/// });
/// let resp = app
///     .oneshot(Request::builder().uri("/scripts").body(Body::empty()).unwrap())
///     .await
///     .unwrap();
/// assert_eq!(resp.status(), StatusCode::OK);
/// # }
/// ```
pub fn chronon_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    ChrononState: FromRef<S>,
{
    Router::new()
        .route("/jobs", get(handlers::list_jobs))
        .route("/jobs/upsert", post(handlers::upsert_job))
        .route("/jobs/pause", post(handlers::pause_job))
        .route("/jobs/resume", post(handlers::resume_job))
        .route("/jobs/run_now", post(handlers::run_now))
        .route("/jobs/{id}", get(handlers::get_job))
        .route("/jobs/{id}/revisions", get(handlers::get_job_revisions))
        .route("/runs", get(handlers::list_runs))
        .route("/runs/{id}", get(handlers::get_run))
        .route("/scripts", get(handlers::list_scripts))
}
