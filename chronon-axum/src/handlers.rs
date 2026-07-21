//! Axum route handlers for `/api/chronon/*`.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use chronon_core::{ChrononError, Job, ScheduleKind};

use crate::dto::{
    JobActionRequest, JobResponse, ListJobsQuery, ListRunsQuery, RunResponse, UpsertJobRequest,
};
use crate::handlers_common::{chronon_err, ApiResponse};
use crate::state::ChrononState;
use chronon_scheduler::CronExpr;

/// `POST /jobs/upsert` — create or update a job; 400 if script missing or cron invalid.
#[tracing::instrument(skip(state, req), fields(job_name = %req.job_name, script_name = %req.script_name))]
pub async fn upsert_job(
    State(state): State<ChrononState>,
    Json(req): Json<UpsertJobRequest>,
) -> (StatusCode, Json<ApiResponse<JobResponse>>) {
    if !state.registry.contains(&req.script_name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err(format!(
                "Script '{}' not found",
                req.script_name
            ))),
        );
    }

    let mut job = Job::new(&req.job_name, &req.script_name);
    job.enabled = req.enabled;
    job.schedule_kind = req.schedule_kind.into();
    job.cron_expr = req.cron_expr.clone();
    job.timezone = req.timezone.clone();
    job.params_json = req.params.clone();
    job.concurrency = req.concurrency;
    job.timeout_ms = req.timeout_ms;
    if let Some(ref actor) = req.actor_json {
        if !actor.is_null() {
            job.actor_json = actor.clone();
        }
    }
    if let Some(ref policy) = req.retry_policy {
        if !policy.is_null() {
            job.retry_policy_json = policy.clone();
        }
    }
    if let Some(ref policy) = req.misfire_policy {
        if !policy.is_null() {
            job.misfire_policy_json = policy.clone();
        }
    }

    if job.schedule_kind == ScheduleKind::Cron {
        if let Some(ref cron_expr) = job.cron_expr {
            match CronExpr::parse(cron_expr, job.timezone.as_deref()) {
                Ok(cron) => job.next_run_at = cron.next_from_now(),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::err(format!("Invalid cron expression: {e}"))),
                    );
                }
            }
        }
    }

    match state.coordinator.upsert_job(job.clone()).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(job.into()))),
        Err(e) => chronon_err(&e),
    }
}

/// `GET /jobs` — list jobs with optional filters and pagination.
pub async fn list_jobs(
    State(state): State<ChrononState>,
    Query(query): Query<ListJobsQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<JobResponse>>>) {
    let jobs = match state.coordinator.list_jobs().await {
        Ok(jobs) => jobs,
        Err(e) => return chronon_err(&e),
    };

    if let Some(ref kind) = query.schedule_kind {
        let ok = matches!(kind.as_str(), "cron" | "run_once" | "manual");
        if !ok {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::err(format!(
                    "invalid schedule_kind '{kind}' (expected cron, run_once, or manual)"
                ))),
            );
        }
    }

    let mut filtered: Vec<Job> = jobs
        .into_iter()
        .filter(|j| {
            query.job_name.as_ref().is_none_or(|n| j.job_name == *n)
                && query
                    .script_name
                    .as_ref()
                    .is_none_or(|n| j.script_name == *n)
                && query.enabled.is_none_or(|e| j.enabled == e)
                && query
                    .schedule_kind
                    .as_ref()
                    .is_none_or(|k| match j.schedule_kind {
                        ScheduleKind::Cron => k == "cron",
                        ScheduleKind::RunOnce => k == "run_once",
                        ScheduleKind::Manual => k == "manual",
                    })
        })
        .collect();

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100).min(1000);
    if offset >= filtered.len() {
        filtered.clear();
    } else {
        let end = (offset + limit).min(filtered.len());
        filtered = filtered[offset..end].to_vec();
    }

    let responses: Vec<JobResponse> = filtered.into_iter().map(Into::into).collect();
    (StatusCode::OK, Json(ApiResponse::ok(responses)))
}

/// `GET /jobs/{id}` — fetch one job by id; 404 when missing.
pub async fn get_job(
    State(state): State<ChrononState>,
    Path(job_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<JobResponse>>) {
    match state.coordinator.get_job(&job_id).await {
        Some(job) => (StatusCode::OK, Json(ApiResponse::ok(job.into()))),
        None => chronon_err(&ChrononError::JobNotFound(job_id)),
    }
}

/// `POST /jobs/pause` — disable scheduling for a job.
pub async fn pause_job(
    State(state): State<ChrononState>,
    Json(req): Json<JobActionRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match state.coordinator.pause_job(&req.job_id).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(e) => chronon_err(&e),
    }
}

/// `POST /jobs/resume` — re-enable scheduling for a job.
pub async fn resume_job(
    State(state): State<ChrononState>,
    Json(req): Json<JobActionRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match state.coordinator.resume_job(&req.job_id).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(e) => chronon_err(&e),
    }
}

/// `POST /jobs/run_now` — enqueue an immediate run; returns new `run_id` in `data`.
#[tracing::instrument(skip(state, req), fields(job_id = %req.job_id))]
pub async fn run_now(
    State(state): State<ChrononState>,
    Json(req): Json<JobActionRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match state
        .coordinator
        .run_now_with_params(&req.job_id, req.params)
        .await
    {
        Ok(run_id) => (StatusCode::OK, Json(ApiResponse::ok(run_id))),
        Err(e) => chronon_err(&e),
    }
}

/// `GET /jobs/{id}/revisions` — revision history as JSON objects.
pub async fn get_job_revisions(
    State(state): State<ChrononState>,
    Path(job_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<Vec<serde_json::Value>>>) {
    match state.coordinator.list_revisions(&job_id).await {
        Ok(revisions) => {
            let json_revisions: Vec<serde_json::Value> = revisions
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "revision_id": r.revision_id,
                        "revision_number": r.revision_number,
                        "changed_at": r.changed_at.to_rfc3339(),
                        "changed_by_actor_json": r.changed_by_actor_json,
                        "snapshot_json": r.snapshot_json,
                    })
                })
                .collect();
            (StatusCode::OK, Json(ApiResponse::ok(json_revisions)))
        }
        Err(e) => chronon_err(&e),
    }
}

/// `GET /runs` — paginated run list with optional filters.
pub async fn list_runs(
    State(state): State<ChrononState>,
    Query(query): Query<ListRunsQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<RunResponse>>>) {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    match state
        .coordinator
        .list_runs(
            query.job_id.as_deref(),
            query.status.as_deref(),
            offset,
            limit,
        )
        .await
    {
        Ok(runs) => {
            let responses: Vec<RunResponse> = runs.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(ApiResponse::ok(responses)))
        }
        Err(e) => chronon_err(&e),
    }
}

/// `GET /runs/{id}` — fetch one run; 404 when missing.
pub async fn get_run(
    State(state): State<ChrononState>,
    Path(run_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<RunResponse>>) {
    match state.coordinator.get_run(&run_id).await {
        Ok(Some(run)) => (StatusCode::OK, Json(ApiResponse::ok(run.into()))),
        Ok(None) => chronon_err(&ChrononError::RunNotFound(run_id)),
        Err(e) => chronon_err(&e),
    }
}

/// `GET /scripts` — list registered scripts from the host registry.
pub async fn list_scripts(
    State(state): State<ChrononState>,
) -> Json<ApiResponse<Vec<crate::dto::ScriptResponse>>> {
    let scripts: Vec<crate::dto::ScriptResponse> = state
        .registry
        .list()
        .into_iter()
        .map(|d| crate::dto::ScriptResponse {
            name: d.name.to_string(),
            signature_json: d.signature_json.to_string(),
            signature_hash: d.signature_hash,
        })
        .collect();
    Json(ApiResponse::ok(scripts))
}
