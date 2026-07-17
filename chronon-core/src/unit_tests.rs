use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    use crate::context::{ContextFactory, NoOpContextFactory, ScriptContext};
    use crate::error::Result;
    use crate::models::{
        Job, JobRevision, PartitionAssignment, Run, RunStatus, SchedulerLeader, Script, Worker,
    };
    use crate::router::{StoreRouter, DEFAULT_STORE_NAME};
    use crate::store::SchedulerStore;
    use crate::ChrononError;

    struct EmptyStore;

    #[async_trait]
    impl SchedulerStore for EmptyStore {
        async fn upsert_job(&self, _: &Job) -> Result<()> {
            Ok(())
        }
        async fn get_job(&self, _: &str) -> Result<Option<Job>> {
            Ok(None)
        }
        async fn get_job_by_name(&self, _: &str) -> Result<Option<Job>> {
            Ok(None)
        }
        async fn list_jobs(&self) -> Result<Vec<Job>> {
            Ok(vec![])
        }
        async fn list_due_jobs(&self, _: DateTime<Utc>) -> Result<Vec<Job>> {
            Ok(vec![])
        }
        async fn pause_job(&self, _: &str) -> Result<()> {
            Ok(())
        }
        async fn resume_job(&self, _: &str) -> Result<()> {
            Ok(())
        }
        async fn create_run(&self, _: &Run) -> Result<()> {
            Ok(())
        }
        async fn update_run(&self, _: &Run) -> Result<()> {
            Ok(())
        }
        async fn get_run(&self, _: &str) -> Result<Option<Run>> {
            Ok(None)
        }
        async fn list_runs_for_job(&self, _: &str, _: usize) -> Result<Vec<Run>> {
            Ok(vec![])
        }
        async fn list_runs_filtered(
            &self,
            _: Option<&str>,
            _: Option<RunStatus>,
            _: usize,
            _: usize,
        ) -> Result<Vec<Run>> {
            Ok(vec![])
        }
        async fn claim_next_queued(
            &self,
            _: &str,
            _: &str,
            _: DateTime<Utc>,
            _: i64,
        ) -> Result<Option<Run>> {
            Ok(None)
        }
        async fn claim_run_by_id(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: DateTime<Utc>,
            _: i64,
        ) -> Result<Option<Run>> {
            Ok(None)
        }
        async fn renew_run_lease(
            &self,
            _: &str,
            _: &str,
            _: DateTime<Utc>,
            _: i64,
        ) -> Result<bool> {
            Ok(false)
        }
        async fn append_revision(&self, _: &JobRevision) -> Result<()> {
            Ok(())
        }
        async fn list_revisions(&self, _: &str) -> Result<Vec<JobRevision>> {
            Ok(vec![])
        }
        async fn upsert_script(&self, _: &Script) -> Result<()> {
            Ok(())
        }
        async fn get_script(&self, _: &str) -> Result<Option<Script>> {
            Ok(None)
        }
        async fn try_claim_run_once(
            &self,
            _: &str,
            _: &str,
            _: DateTime<Utc>,
            _: i64,
        ) -> Result<bool> {
            Ok(false)
        }
        async fn mark_run_once_completed(&self, _: &str, _: DateTime<Utc>) -> Result<()> {
            Ok(())
        }
        async fn release_run_once_claim(
            &self,
            _: &str,
            _: &str,
            _: DateTime<Utc>,
        ) -> Result<()> {
            Ok(())
        }
        async fn find_due_job_ids_in_partitions(
            &self,
            _: &[u32],
            _: DateTime<Utc>,
            _: u32,
        ) -> Result<Vec<String>> {
            Ok(vec![])
        }
        async fn min_next_run_at_in_partitions(
            &self,
            _: &[u32],
        ) -> Result<Option<DateTime<Utc>>> {
            Ok(None)
        }
        async fn claim_job_for_tick(
            &self,
            _: &str,
            _: &str,
            _: DateTime<Utc>,
            _: i64,
        ) -> Result<bool> {
            Ok(false)
        }
        async fn release_job_tick_claim(&self, _: &str) -> Result<()> {
            Ok(())
        }
        async fn persist_post_tick_job_state(
            &self,
            _: &str,
            _: Option<DateTime<Utc>>,
        ) -> Result<()> {
            Ok(())
        }
        async fn try_acquire_leader(&self, _: &str, _: i64) -> Result<bool> {
            Ok(false)
        }
        async fn renew_leader_lease(&self, _: &str, _: i64) -> Result<()> {
            Ok(())
        }
        async fn get_leader(&self) -> Result<Option<SchedulerLeader>> {
            Ok(None)
        }
        async fn upsert_partition_assignment(&self, _: &PartitionAssignment) -> Result<()> {
            Ok(())
        }
        async fn list_partition_assignments(&self) -> Result<Vec<PartitionAssignment>> {
            Ok(vec![])
        }
        async fn register_worker(&self, _: &Worker) -> Result<()> {
            Ok(())
        }
        async fn heartbeat_worker(&self, _: &str, _: DateTime<Utc>) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn job_new_has_defaults() {
        let job = Job::new("daily", "cleanup");
        assert!(job.enabled);
        assert_eq!(job.current_revision, 1);
    }

    #[test]
    fn run_status_helpers() {
        assert!(RunStatus::Success.is_terminal());
        assert!(!RunStatus::Queued.is_terminal());
        assert!(RunStatus::Queued.is_active());
    }

    #[test]
    fn run_lifecycle() {
        let mut run = Run::for_job("job-1", "script", Utc::now());
        run.start();
        assert_eq!(run.status, RunStatus::Running);
        run.complete();
        assert!(run.duration_ms.is_some());
    }

    #[test]
    fn store_router_register_and_get() {
        let store = Arc::new(EmptyStore);
        let mut router = StoreRouter::new();
        router.register(DEFAULT_STORE_NAME, store);
        assert!(router.get(DEFAULT_STORE_NAME).is_some());
    }

    #[test]
    fn store_router_missing_default_is_none() {
        let router = StoreRouter::new();
        assert!(router.get(DEFAULT_STORE_NAME).is_none());
    }

    #[test]
    fn noop_context_factory() {
        let factory = NoOpContextFactory;
        let ctx = factory.build(&json!({"actor": "test"})).expect("build");
        assert_eq!(ctx.label(), "noop");
        let _: Box<dyn ScriptContext> = ctx;
    }

    #[test]
    fn identity_error_maps_to_chronon_error() {
        let err: ChrononError = crate::IdentityError("bad actor".into()).into();
        assert!(matches!(err, ChrononError::Internal(_)));
    }

    #[test]
    fn job_json_roundtrip() {
        let job = Job::new("nightly", "cleanup");
        let json = serde_json::to_string(&job).expect("serialize");
        let back: Job = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.job_name, "nightly");
    }
