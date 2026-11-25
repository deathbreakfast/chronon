//! [`SchedulerStore`] contract for child sets, shard sets, run outputs and active-run counts.

use chrono::{Duration, Utc};
use chronon_core::models::{ChildOutcome, ChildSet, Job, Run, RunStatus, ScheduleKind, ShardSet};
use chronon_core::store::SchedulerStore;
use chronon_core::{ChrononError, Result};
use serde_json::json;

/// Port contract for the child-set and shard-set store methods every adapter must satisfy.
///
/// Expects an empty store.
pub async fn run_child_shard_contract(store: &dyn SchedulerStore) -> Result<()> {
    child_jobs_round_trip_and_are_never_due(store).await?;
    child_set_with_runs_round_trips_and_runs_are_claimable(store).await?;
    child_set_rolls_back_when_a_run_fails(store).await?;
    record_child_result_counts_and_settles(store).await?;
    shard_set_compare_and_set(store).await?;
    shard_checkpoints_merge(store).await?;
    run_outputs_survive_update_run(store).await?;
    count_active_runs_counts_claimed_and_running(store).await?;
    Ok(())
}

fn child_run(job_id: &str, set_id: &str, parent_run_id: &str, index: i32) -> Run {
    let mut run = Run::for_job(job_id, "child-script", Utc::now() - Duration::seconds(1));
    run.parent_run_id = Some(parent_run_id.to_string());
    run.root_run_id = Some(parent_run_id.to_string());
    run.child_index = Some(index);
    run.child_set_id = Some(set_id.to_string());
    run.run_values_json = json!({"child": {"index": index}});
    run.pool_id = Some("children".into());
    run
}

async fn child_jobs_round_trip_and_are_never_due(store: &dyn SchedulerStore) -> Result<()> {
    let mut job = Job::new("contract-child-job", "child-script");
    job.schedule_kind = ScheduleKind::Child {
        parent_job_id: "contract-parent".into(),
    };
    // Even with a past next_run_at and a partition, a child job is never due on a tick.
    job.next_run_at = Some(Utc::now() - Duration::seconds(5));
    job.partition_hash = Some(7);
    store.upsert_job(&job).await?;
    let back = store.get_job(&job.job_id).await?.expect("child job");
    assert_eq!(back.schedule_kind, job.schedule_kind);
    let due = store
        .find_due_job_ids_in_partitions(&[7], Utc::now(), 100)
        .await?;
    assert!(!due.contains(&job.job_id), "child job must not be tick-due");
    assert_eq!(store.min_next_run_at_in_partitions(&[7]).await?, None);
    assert!(store.delete_job(&job.job_id).await?);
    Ok(())
}

async fn child_set_with_runs_round_trips_and_runs_are_claimable(
    store: &dyn SchedulerStore,
) -> Result<()> {
    let set = ChildSet::new("contract-parent-run", "contract-child-job-id", 2);
    let runs = [
        child_run(
            "contract-child-job-id",
            &set.set_id,
            "contract-parent-run",
            0,
        ),
        child_run(
            "contract-child-job-id",
            &set.set_id,
            "contract-parent-run",
            1,
        ),
    ];
    store.create_child_set_with_runs(&set, &runs).await?;

    let stored = store.get_child_set(&set.set_id).await?.expect("set");
    assert_eq!(stored.count, 2);
    assert_eq!(stored.finished(), 0);
    assert_eq!(stored.parent_run_id, "contract-parent-run");
    assert_eq!(stored.child_job_id, "contract-child-job-id");
    assert!(store.get_child_set("missing-set").await?.is_none());

    let listed = store.list_child_sets_for_run("contract-parent-run").await?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].set_id, set.set_id);
    assert_eq!(
        store.list_child_sets_for_run("no-such-parent").await?,
        vec![]
    );

    for run in &runs {
        let back = store.get_run(&run.run_id).await?.expect("child run");
        assert_eq!(back.child_set_id.as_deref(), Some(set.set_id.as_str()));
        assert_eq!(back.run_values_json, run.run_values_json);
        assert_eq!(back.outputs_json, json!({}));
    }

    let mut claimed = Vec::new();
    while let Some(run) = store
        .claim_next_queued("children", "contract-worker", Utc::now(), 30)
        .await?
    {
        claimed.push(run.run_id);
    }
    claimed.sort();
    let mut expected: Vec<_> = runs.iter().map(|r| r.run_id.clone()).collect();
    expected.sort();
    assert_eq!(claimed, expected, "every child run is claimable once");
    Ok(())
}

async fn child_set_rolls_back_when_a_run_fails(store: &dyn SchedulerStore) -> Result<()> {
    let existing = Run::for_job("contract-rollback-job", "child-script", Utc::now());
    store.create_run(&existing).await?;

    let set = ChildSet::new("contract-rollback-parent", "contract-rollback-job", 2);
    let fresh = child_run(
        "contract-rollback-job",
        &set.set_id,
        "contract-rollback-parent",
        0,
    );
    let mut duplicate = child_run(
        "contract-rollback-job",
        &set.set_id,
        "contract-rollback-parent",
        1,
    );
    duplicate.run_id.clone_from(&existing.run_id);

    let result = store
        .create_child_set_with_runs(&set, &[fresh.clone(), duplicate])
        .await;
    assert!(
        matches!(result, Err(ChrononError::StorageError { .. })),
        "duplicate run_id must fail: {result:?}"
    );
    assert!(store.get_child_set(&set.set_id).await?.is_none());
    assert!(store.get_run(&fresh.run_id).await?.is_none());
    let untouched = store.get_run(&existing.run_id).await?.expect("existing");
    assert!(untouched.child_set_id.is_none());
    Ok(())
}

async fn record_child_result_counts_and_settles(store: &dyn SchedulerStore) -> Result<()> {
    let set = ChildSet::new("contract-count-parent", "contract-count-job", 3);
    store.create_child_set_with_runs(&set, &[]).await?;

    let after = store
        .record_child_result(&set.set_id, ChildOutcome::Succeeded)
        .await?;
    assert_eq!((after.succeeded, after.failed, after.skipped), (1, 0, 0));
    let after = store
        .record_child_result(&set.set_id, ChildOutcome::Failed)
        .await?;
    assert_eq!((after.succeeded, after.failed, after.skipped), (1, 1, 0));
    let after = store
        .record_child_result(&set.set_id, ChildOutcome::Skipped)
        .await?;
    assert_eq!(after.finished(), after.count);
    assert!(after.settled_at.is_none());

    assert!(store
        .record_child_result("missing-set", ChildOutcome::Succeeded)
        .await
        .is_err());

    let at = Utc::now();
    store.settle_child_set(&set.set_id, at).await?;
    let settled = store.get_child_set(&set.set_id).await?.expect("set");
    let first = settled.settled_at.expect("settled");
    assert!((first - at).num_milliseconds().abs() < 1_000);
    // Settling again keeps the first timestamp.
    store
        .settle_child_set(&set.set_id, at + Duration::hours(1))
        .await?;
    let again = store.get_child_set(&set.set_id).await?.expect("set");
    assert_eq!(again.settled_at, Some(first));
    store.settle_child_set("missing-set", at).await?;
    Ok(())
}

async fn shard_set_compare_and_set(store: &dyn SchedulerStore) -> Result<()> {
    let mut set = ShardSet::new(
        "contract-shard-parent",
        "contract-shard-job",
        "id",
        json!(0),
        100,
    );
    set.end = Some(json!(1_000));
    store.create_shard_set(&set).await?;
    assert!(
        store.create_shard_set(&set).await.is_err(),
        "duplicate set_id"
    );

    let stored = store.get_shard_set(&set.set_id).await?.expect("shard set");
    assert_eq!(stored.next_start, json!(0));
    assert_eq!(stored.end, Some(json!(1_000)));
    assert_eq!(stored.rows_per_shard, 100);
    assert!(!stored.paused);
    assert_eq!(stored.checkpoints_json, json!({}));
    assert!(store.get_shard_set("missing-set").await?.is_none());

    let mut next = stored.clone();
    next.next_start = json!(100);
    next.paused = true;
    assert!(store.update_shard_set(&next, &json!(0)).await?);
    // A second writer that read the old next_start loses.
    let mut stale = stored.clone();
    stale.next_start = json!(200);
    assert!(!store.update_shard_set(&stale, &json!(0)).await?);

    let after = store.get_shard_set(&set.set_id).await?.expect("shard set");
    assert_eq!(after.next_start, json!(100));
    assert!(after.paused);
    assert!(
        !store
            .update_shard_set(&ShardSet::new("p", "j", "id", json!(1), 1), &json!(1))
            .await?
    );
    Ok(())
}

async fn shard_checkpoints_merge(store: &dyn SchedulerStore) -> Result<()> {
    let set = ShardSet::new(
        "contract-ckpt-parent",
        "contract-ckpt-job",
        "id",
        json!(0),
        10,
    );
    store.create_shard_set(&set).await?;
    store
        .set_shard_checkpoint(&set.set_id, 0, &json!({"last_id": 4}))
        .await?;
    store
        .set_shard_checkpoint(&set.set_id, 3, &json!({"last_id": 37}))
        .await?;
    store
        .set_shard_checkpoint(&set.set_id, 0, &json!({"last_id": 9}))
        .await?;
    let stored = store.get_shard_set(&set.set_id).await?.expect("shard set");
    assert_eq!(
        stored.checkpoints_json,
        json!({"0": {"last_id": 9}, "3": {"last_id": 37}})
    );
    assert!(store
        .set_shard_checkpoint("missing-set", 0, &json!(1))
        .await
        .is_err());
    Ok(())
}

async fn run_outputs_survive_update_run(store: &dyn SchedulerStore) -> Result<()> {
    let run = Run::for_job("contract-outputs-job", "s", Utc::now());
    store.create_run(&run).await?;
    store
        .set_run_outputs(&run.run_id, &json!({"cursor": 42}))
        .await?;
    // A worker still holding the copy it claimed finishes the run.
    let mut stale = run.clone();
    stale.status = RunStatus::Success;
    store.update_run(&stale).await?;
    let back = store.get_run(&run.run_id).await?.expect("run");
    assert_eq!(back.outputs_json, json!({"cursor": 42}));
    assert_eq!(back.status, RunStatus::Success);
    assert!(matches!(
        store.set_run_outputs("missing-run", &json!({})).await,
        Err(ChrononError::RunNotFound(_))
    ));
    Ok(())
}

async fn count_active_runs_counts_claimed_and_running(store: &dyn SchedulerStore) -> Result<()> {
    let job_id = "contract-active-job";
    for status in [
        RunStatus::Queued,
        RunStatus::Claimed,
        RunStatus::Running,
        RunStatus::Running,
        RunStatus::Success,
        RunStatus::Failed,
    ] {
        let mut run = Run::for_job(job_id, "s", Utc::now());
        run.pool_id = Some("contract-active-pool".into());
        run.status = status;
        store.create_run(&run).await?;
    }
    let mut other = Run::for_job("contract-other-job", "s", Utc::now());
    other.status = RunStatus::Running;
    store.create_run(&other).await?;

    assert_eq!(store.count_active_runs_for_job(job_id).await?, 3);
    assert_eq!(store.count_active_runs_for_job("no-such-job").await?, 0);
    Ok(())
}
