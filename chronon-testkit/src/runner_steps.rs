//! Scenario step dispatch helpers for [`super::ScenarioRunner`].

use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::{Duration as ChronoDuration, Utc};
use chronon_scheduler::TickResult;

use crate::bootstrap::BootstrapSession;
use crate::fixtures::{
    count_runs_for_job, counting_probe_total, reset_counting_probe, smoke_actor_json,
    upsert_future_cron_job, upsert_immediate_cron_job, upsert_immediate_run_once_job,
    wait_for_run_terminal,
};
use crate::runner_types::{RunMode, StepTiming};
use crate::scenario::ScenarioStep;

pub(super) async fn run_mutation_step(
    session: &mut BootstrapSession,
    step_index: usize,
    step: &ScenarioStep,
    mode: RunMode,
    step_timings: &mut Vec<StepTiming>,
    last_tick: &mut Option<TickResult>,
) -> Result<()> {
    match step {
        ScenarioStep::RegisterScript { probe } => {
            session.register_probe(*probe);
            if matches!(probe, crate::scenario::ScriptProbeKind::Counting) {
                reset_counting_probe();
            }
        }
        ScenarioStep::ResetCountingProbe => reset_counting_probe(),
        ScenarioStep::UpsertDueCronJob {
            job_name,
            script_name,
            cron,
        } => {
            upsert_cron_job(session, job_name, script_name, cron, false).await?;
        }
        ScenarioStep::UpsertFutureCronJob {
            job_name,
            script_name,
            cron,
        } => {
            upsert_cron_job(session, job_name, script_name, cron, true).await?;
        }
        ScenarioStep::UpsertManualJob {
            job_name,
            script_name,
        } => upsert_manual_job_step(session, job_name, script_name).await?,
        ScenarioStep::UpsertRunOnceDueJob {
            job_name,
            script_name,
        } => {
            let store = session.store_dyn()?;
            let mut job =
                upsert_immediate_run_once_job(store.as_ref(), job_name, script_name).await?;
            job.actor_json = smoke_actor_json();
            store.upsert_job(&job).await?;
        }
        ScenarioStep::InitPartitions => session.init_partitions().await?,
        ScenarioStep::Tick => {
            let start = Instant::now();
            let tick = session.ensure_chronon()?.tick_once().await?;
            step_timings.push(StepTiming {
                step_index,
                op: "tick".into(),
                samples_ms: vec![start.elapsed().as_secs_f64() * 1000.0],
            });
            *last_tick = Some(tick);
        }
        ScenarioStep::PauseJob { job_name } | ScenarioStep::ResumeJob { job_name } => {
            job_action(
                session,
                job_name,
                matches!(step, ScenarioStep::PauseJob { .. }),
            )
            .await?;
        }
        ScenarioStep::RunNow { job_name } => {
            let chronon = session.ensure_chronon()?;
            let job = job_by_name(chronon, job_name).await?;
            chronon.coordinator.run_now(&job.job_id).await?;
        }
        ScenarioStep::SpawnEmbedded => session.spawn_embedded().await?,
        ScenarioStep::ShutdownEmbedded => session.shutdown_embedded().await?,
        ScenarioStep::WaitRunTerminal {
            job_name,
            status,
            timeout_ms,
        } => {
            let store = session.store_dyn()?;
            let start = Instant::now();
            wait_for_run_terminal(store, job_name, *status, Duration::from_millis(*timeout_ms))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if mode == RunMode::Benchmark {
                step_timings.push(StepTiming {
                    step_index,
                    op: "wait_run_terminal".into(),
                    samples_ms: vec![start.elapsed().as_secs_f64() * 1000.0],
                });
            }
        }
        _ => bail!("unexpected step in mutation dispatch"),
    }
    Ok(())
}

pub(super) async fn run_assertion_step(
    session: &BootstrapSession,
    step: &ScenarioStep,
    mode: RunMode,
    last_tick: Option<&TickResult>,
) -> Result<()> {
    if mode == RunMode::Benchmark {
        return Ok(());
    }
    match step {
        ScenarioStep::AssertRunCount { job_name, expected } => {
            let store = session.store_dyn()?;
            let count = count_runs_for_job(store.as_ref(), job_name).await?;
            if count != *expected as usize {
                bail!("expected {expected} runs for {job_name}, got {count}");
            }
        }
        ScenarioStep::AssertTelemetryCounter { name, labels, min } => {
            let label_refs: Vec<(&str, &str)> = labels
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let hits = session
                .telemetry()
                .recorded_counters_matching(name, &label_refs);
            let total: u64 = hits.iter().map(|h| h.delta).sum();
            if total < *min {
                bail!("expected counter {name} >= {min}, got {total} (labels {labels:?})");
            }
        }
        ScenarioStep::AssertLastTickEnqueued { expected } => {
            let tick = last_tick.ok_or_else(|| anyhow::anyhow!("no tick executed yet"))?;
            if tick.enqueued != *expected {
                bail!(
                    "expected last tick to enqueue {expected}, got {}",
                    tick.enqueued
                );
            }
        }
        ScenarioStep::AssertPartitionDueFilter { job_name } => {
            assert_partition_due_filter(session, job_name).await?;
        }
        ScenarioStep::AssertRevisionCount { job_name, min } => {
            let store = session.store_dyn()?;
            let job = store
                .get_job_by_name(job_name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("job {job_name} not found"))?;
            let revisions = store.list_revisions(&job.job_id).await?;
            if revisions.len() < *min as usize {
                bail!(
                    "expected >= {min} revisions for {job_name}, got {}",
                    revisions.len()
                );
            }
        }
        ScenarioStep::AssertJobNotDue { job_name } => {
            assert_job_not_due(session, job_name).await?;
        }
        ScenarioStep::AssertCountingProbe { min } => {
            let total = counting_probe_total();
            if total < *min as usize {
                bail!("expected counting probe >= {min}, got {total}");
            }
        }
        _ => bail!("unexpected step in assertion dispatch"),
    }
    Ok(())
}

pub(super) fn is_assertion_step(step: &ScenarioStep) -> bool {
    matches!(
        step,
        ScenarioStep::AssertRunCount { .. }
            | ScenarioStep::AssertTelemetryCounter { .. }
            | ScenarioStep::AssertLastTickEnqueued { .. }
            | ScenarioStep::AssertPartitionDueFilter { .. }
            | ScenarioStep::AssertRevisionCount { .. }
            | ScenarioStep::AssertJobNotDue { .. }
            | ScenarioStep::AssertCountingProbe { .. }
    )
}

async fn upsert_cron_job(
    session: &BootstrapSession,
    job_name: &str,
    script_name: &str,
    cron: &str,
    future: bool,
) -> Result<()> {
    let store = session.store_dyn()?;
    let mut job = if future {
        upsert_future_cron_job(store.as_ref(), job_name, script_name, cron).await?
    } else {
        upsert_immediate_cron_job(store.as_ref(), job_name, script_name, cron).await?
    };
    job.actor_json = smoke_actor_json();
    store.upsert_job(&job).await?;
    Ok(())
}

async fn upsert_manual_job_step(
    session: &mut BootstrapSession,
    job_name: &str,
    script_name: &str,
) -> Result<()> {
    use chronon_core::models::{Job, ScheduleKind};
    use chronon_scheduler::partition_hash_i64_for_job_id;

    let chronon = session.ensure_chronon()?;
    let mut job = Job::new(job_name, script_name);
    job.schedule_kind = ScheduleKind::Manual;
    job.actor_json = smoke_actor_json();
    job.partition_hash = Some(partition_hash_i64_for_job_id(&job.job_id));
    chronon.coordinator.upsert_job(job).await?;
    Ok(())
}

async fn job_by_name(
    chronon: &chronon_runtime::Chronon,
    job_name: &str,
) -> Result<chronon_core::models::Job> {
    chronon
        .coordinator
        .get_job_by_name(job_name)
        .await
        .ok_or_else(|| anyhow::anyhow!("job {job_name} not found"))
}

async fn job_action(session: &mut BootstrapSession, job_name: &str, pause: bool) -> Result<()> {
    let chronon = session.ensure_chronon()?;
    let job = job_by_name(chronon, job_name).await?;
    if pause {
        chronon.coordinator.pause_job(&job.job_id).await?;
    } else {
        chronon.coordinator.resume_job(&job.job_id).await?;
    }
    Ok(())
}

async fn assert_partition_due_filter(session: &BootstrapSession, job_name: &str) -> Result<()> {
    let store = session.store_dyn()?;
    let job = store
        .get_job_by_name(job_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("job {job_name} not found"))?;
    let partition = job.partition_hash.unwrap_or(0).max(0) as u32;
    let until = Utc::now() + ChronoDuration::milliseconds(50);
    let due = store
        .find_due_job_ids_in_partitions(&[partition], until, 50)
        .await?;
    if !due.iter().any(|id| id == &job.job_id) {
        bail!("expected job in partition {partition} due set");
    }
    let others: Vec<u32> = (0..4).filter(|p| *p != partition).collect();
    let other_due = store
        .find_due_job_ids_in_partitions(&others, until, 50)
        .await?;
    if other_due.iter().any(|id| id == &job.job_id) {
        bail!("job unexpectedly due in non-owning partitions");
    }
    Ok(())
}

async fn assert_job_not_due(session: &BootstrapSession, job_name: &str) -> Result<()> {
    let store = session.store_dyn()?;
    let job = store
        .get_job_by_name(job_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("job {job_name} not found"))?;
    let partition = job.partition_hash.unwrap_or(0).max(0) as u32;
    let due = store
        .find_due_job_ids_in_partitions(&[partition], Utc::now(), 50)
        .await?;
    if due.iter().any(|id| id == &job.job_id) {
        bail!("expected job {job_name} to be not due");
    }
    Ok(())
}
