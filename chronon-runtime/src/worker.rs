//! Worker loop: claim queued runs and execute scripts.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use chronon_core::models::{RunStatus, Worker, WorkerStatus};
use chronon_core::store::SchedulerStore;
use chronon_executor::{execute_script, ExecuteScriptRequest, Executor};
use chronon_scheduler::{
    run_worker_lease_renew_secs, run_worker_lease_ttl_secs, worker_concurrency_from_env,
};
use chronon_telemetry::TelemetrySink;
use tokio::sync::Notify;
use tokio::time::{sleep, timeout};
use tracing::Instrument;

use crate::env::env_flag;
use crate::retry::finalize_failed_run;

fn worker_row_id(worker_id: &str) -> String {
    let mut s: String = worker_id
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(200)
        .collect();
    if s.is_empty() {
        s = "worker".to_string();
    }
    s
}

async fn worker_slot(
    store: Arc<dyn SchedulerStore>,
    executor: Arc<Executor>,
    telemetry: Arc<dyn TelemetrySink>,
    pool: String,
    worker_id: String,
) {
    loop {
        if env_flag("CHRONON_DISABLE_WORKER") || env_flag("CHRONON_INTERACTIVE") {
            sleep(Duration::from_secs(30)).await;
            continue;
        }

        let now = Utc::now();
        let ttl = run_worker_lease_ttl_secs();
        let Some(mut run) = store
            .claim_next_queued(&pool, &worker_id, now, ttl)
            .await
            .ok()
            .flatten()
        else {
            sleep(Duration::from_millis(50)).await;
            continue;
        };

        let Some(job_id) = run.job_id.clone() else {
            run.fail("missing job_id");
            let _ = store.update_run(&run).await;
            continue;
        };
        let Some(job) = store.get_job(&job_id).await.ok().flatten() else {
            run.fail("job not found");
            let _ = store.update_run(&run).await;
            continue;
        };

        let run_span = tracing::info_span!(
            "worker_run",
            run_id = %run.run_id,
            worker_id = %worker_id,
            pool = %pool,
            job_name = %job.job_name,
            script_name = %run.script_name,
        );

        async {
            tracing::info!("claimed run");

            run.start();
            run.status = RunStatus::Running;
            if store.update_run(&run).await.is_err() {
                return;
            }

            let run_id = run.run_id.clone();
            let renew_store = Arc::clone(&store);
            let wid = worker_id.clone();
            let renew_every = Duration::from_secs(run_worker_lease_renew_secs());
            let renew_handle = tokio::spawn(async move {
                loop {
                    sleep(renew_every).await;
                    let ok = renew_store
                        .renew_run_lease(&run_id, &wid, Utc::now(), run_worker_lease_ttl_secs())
                        .await
                        .unwrap_or(false);
                    if !ok {
                        break;
                    }
                }
            });

            let started = Utc::now();
            let exec_fut = execute_script(ExecuteScriptRequest {
                registry: &executor.registry,
                context_factory: &executor.context_factory,
                telemetry: &executor.telemetry,
                script_name: &run.script_name,
                actor_json: &job.actor_json,
                params_json: run.params_json.clone(),
                job_name: &job.job_name,
                run_id: &run.run_id,
            });

            let res = match job.timeout_ms {
                Some(ms) if ms > 0 => {
                    match timeout(Duration::from_millis(ms as u64), exec_fut).await {
                        Ok(inner) => inner.map_err(|e| (RunStatus::Failed, e.to_string())),
                        Err(_) => {
                            Err((RunStatus::Timeout, format!("run exceeded timeout_ms={ms}")))
                        }
                    }
                }
                _ => exec_fut
                    .await
                    .map_err(|e| (RunStatus::Failed, e.to_string())),
            };
            renew_handle.abort();

            let duration_ms = (Utc::now() - started).num_milliseconds();
            match res {
                Ok(()) => {
                    run.complete();
                    run.duration_ms = Some(duration_ms);
                    telemetry.record_counter(
                        "chronon_runs_completed",
                        &[("job", job.job_name.as_str())],
                        1,
                    );
                    tracing::info!(duration_ms, "worker run completed");
                    let _ = store.update_run(&run).await;
                }
                Err((status, err)) => {
                    telemetry.record_counter(
                        "chronon_runs_failed",
                        &[("job", job.job_name.as_str())],
                        1,
                    );
                    tracing::warn!(duration_ms, error = %err, ?status, "worker run failed");
                    run.duration_ms = Some(duration_ms);
                    finalize_failed_run(&store, run, &job, status, err).await;
                }
            }
        }
        .instrument(run_span)
        .await;
    }
}

async fn heartbeat_worker(store: &Arc<dyn SchedulerStore>, worker_id: &str, pool: &str) {
    let now = Utc::now();
    let row_id = worker_row_id(worker_id);
    let worker = Worker {
        worker_id: row_id.clone(),
        pool_id: pool.to_string(),
        cell_id: None,
        status: WorkerStatus::Online,
        last_heartbeat_at: now,
        capacity_json: None,
        created_at: now,
        updated_at: now,
    };
    if store.register_worker(&worker).await.is_ok() {
        let _ = store.heartbeat_worker(&row_id, now).await;
    }
}

/// Spawn worker slots + heartbeat until `shutdown` is notified.
///
/// Worker id is `{instance_id}:{pool_id}`; concurrency from `CHRONON_WORKER_CONCURRENCY`.
/// Respects `CHRONON_DISABLE_WORKER` by sleeping without claiming runs.
pub async fn run_worker_loop(
    store: Arc<dyn SchedulerStore>,
    executor: Arc<Executor>,
    telemetry: Arc<dyn TelemetrySink>,
    pool_id: String,
    instance_id: String,
    shutdown: Arc<Notify>,
) {
    let worker_id = format!("{instance_id}:{pool_id}");
    let n = worker_concurrency_from_env();

    for i in 0..n {
        let store = Arc::clone(&store);
        let executor = Arc::clone(&executor);
        let telemetry = Arc::clone(&telemetry);
        let pool = pool_id.clone();
        let wid = format!("{worker_id}:{i}");
        tokio::spawn(async move {
            worker_slot(store, executor, telemetry, pool, wid).await;
        });
    }

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            () = shutdown.notified() => break,
            _ = interval.tick() => {
                heartbeat_worker(&store, &worker_id, &pool_id).await;
            }
        }
    }
}
