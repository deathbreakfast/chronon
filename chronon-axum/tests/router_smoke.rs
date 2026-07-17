//! Integration smoke tests for the Chronon Axum router.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use chronon_axum::{
    chronon_router, ApiResponse, ChrononState, JobResponse, RunResponse, ScriptResponse,
};
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_core::models::{Job, Run, RunStatus, ScheduleKind};
use chronon_core::{Result, ScriptContext};
use chronon_executor::{ScriptDescriptor, ScriptRegistry};
use chronon_runtime::CoordinatorService;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn noop_invoke(
    _ctx: Box<dyn ScriptContext>,
    _params: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
    Box::pin(async { Ok(()) })
}

#[derive(Clone)]
struct AppState {
    chronon: ChrononState,
}

impl axum::extract::FromRef<AppState> for ChrononState {
    fn from_ref(state: &AppState) -> Self {
        state.chronon.clone()
    }
}

fn test_state() -> AppState {
    let store = Arc::new(InMemorySchedulerStore::new());
    let coordinator = Arc::new(CoordinatorService::new(store));
    let registry = Arc::new({
        let mut r = ScriptRegistry::new();
        r.register(ScriptDescriptor::new("test_script", noop_invoke));
        r
    });
    AppState {
        chronon: ChrononState::new(coordinator, registry),
    }
}

fn test_app(state: AppState) -> Router {
    chronon_router::<AppState>().with_state(state)
}

async fn json_body<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn seed_job(state: &AppState, job_name: &str) -> Job {
    let mut job = Job::new(job_name, "test_script");
    job.schedule_kind = ScheduleKind::Manual;
    state
        .chronon
        .coordinator
        .upsert_job(job.clone())
        .await
        .unwrap();
    state
        .chronon
        .coordinator
        .get_job_by_name(job_name)
        .await
        .expect("job stored")
}

async fn seed_run(state: &AppState, job: &Job) -> Run {
    let run_id = state
        .chronon
        .coordinator
        .run_now(&job.job_id)
        .await
        .unwrap();
    state
        .chronon
        .coordinator
        .get_run(&run_id)
        .await
        .unwrap()
        .expect("run stored")
}

#[tokio::test]
async fn upsert_job_ok() {
    let state = test_state();
    let app = test_app(state);
    let body = serde_json::json!({
        "job_name": "j1",
        "script_name": "test_script",
        "schedule_kind": "manual",
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/upsert")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<JobResponse> = json_body(resp).await;
    assert!(parsed.success);
    assert_eq!(parsed.data.unwrap().job_name, "j1");
}

#[tokio::test]
async fn upsert_job_script_not_found() {
    let app = test_app(test_state());
    let body = serde_json::json!({
        "job_name": "j1",
        "script_name": "missing",
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/upsert")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upsert_job_invalid_cron() {
    let app = test_app(test_state());
    let body = serde_json::json!({
        "job_name": "j1",
        "script_name": "test_script",
        "cron_expr": "not-a-cron",
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/upsert")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_jobs_ok() {
    let state = test_state();
    seed_job(&state, "listed").await;
    let app = test_app(state);
    let resp = app
        .oneshot(Request::builder().uri("/jobs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<Vec<JobResponse>> = json_body(resp).await;
    assert!(parsed.success);
    assert_eq!(parsed.data.unwrap().len(), 1);
}

#[tokio::test]
async fn list_jobs_filters_and_rejects_bad_schedule_kind() {
    let state = test_state();
    seed_job(&state, "filter-a").await;
    let app = test_app(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/jobs?job_name=filter-a&enabled=true&limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<Vec<JobResponse>> = json_body(resp).await;
    assert_eq!(parsed.data.unwrap().len(), 1);

    let app = test_app(state);
    let bad = app
        .oneshot(
            Request::builder()
                .uri("/jobs?schedule_kind=nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_job_ok() {
    let state = test_state();
    let job = seed_job(&state, "fetch-me").await;
    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/jobs/{}", job.job_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<JobResponse> = json_body(resp).await;
    assert_eq!(parsed.data.unwrap().job_name, "fetch-me");
}

#[tokio::test]
async fn get_job_not_found() {
    let app = test_app(test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/jobs/missing-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pause_and_resume_job_ok() {
    let state = test_state();
    let job = seed_job(&state, "pausable").await;
    let app = test_app(state);
    let pause_body = serde_json::json!({ "job_id": job.job_id });
    let pause_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/pause")
                .header("content-type", "application/json")
                .body(Body::from(pause_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pause_resp.status(), StatusCode::OK);

    let resume_body = serde_json::json!({ "job_id": job.job_id });
    let resume_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/resume")
                .header("content-type", "application/json")
                .body(Body::from(resume_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resume_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn run_now_ok() {
    let state = test_state();
    let job = seed_job(&state, "trigger").await;
    let app = test_app(state);
    let body = serde_json::json!({ "job_id": job.job_id });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/run_now")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<String> = json_body(resp).await;
    assert!(!parsed.data.unwrap().is_empty());
}

#[tokio::test]
async fn run_now_job_not_found() {
    let app = test_app(test_state());
    let body = serde_json::json!({ "job_id": "missing-job" });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs/run_now")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn get_job_revisions_ok() {
    let state = test_state();
    let job = seed_job(&state, "revisions").await;
    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/jobs/{}/revisions", job.job_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<Vec<serde_json::Value>> = json_body(resp).await;
    assert!(!parsed.data.unwrap().is_empty());
}

#[tokio::test]
async fn list_runs_ok() {
    let state = test_state();
    let job = seed_job(&state, "runs-job").await;
    seed_run(&state, &job).await;
    let app = test_app(state);
    let resp = app
        .oneshot(Request::builder().uri("/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<Vec<RunResponse>> = json_body(resp).await;
    assert_eq!(parsed.data.unwrap().len(), 1);
}

#[tokio::test]
async fn get_run_ok() {
    let state = test_state();
    let job = seed_job(&state, "run-fetch").await;
    let run = seed_run(&state, &job).await;
    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/runs/{}", run.run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<RunResponse> = json_body(resp).await;
    assert_eq!(parsed.data.unwrap().run_id, run.run_id);
}

#[tokio::test]
async fn get_run_not_found() {
    let app = test_app(test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/runs/missing-run")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_scripts_ok() {
    let app = test_app(test_state());
    let resp = app
        .oneshot(Request::builder().uri("/scripts").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<Vec<ScriptResponse>> = json_body(resp).await;
    assert!(parsed.success);
    assert_eq!(parsed.data.unwrap().len(), 1);
}

#[tokio::test]
async fn list_runs_filtered_by_status() {
    let state = test_state();
    let job = seed_job(&state, "filter-job").await;
    let mut run = seed_run(&state, &job).await;
    run.status = RunStatus::Success;
    state
        .chronon
        .coordinator
        .store()
        .update_run(&run)
        .await
        .unwrap();
    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/runs?status=success")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let parsed: ApiResponse<Vec<RunResponse>> = json_body(resp).await;
    assert_eq!(parsed.data.unwrap().len(), 1);
}
