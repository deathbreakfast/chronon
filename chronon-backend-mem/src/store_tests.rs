use chronon_core::models::Job;
use chronon_core::store::SchedulerStore;
use chronon_core::default_store_from_global;

use crate::install_default_mem_store;

#[tokio::test]
async fn install_default_mem_store_resolves_via_global_router() {
    let installed = install_default_mem_store();
    let resolved = default_store_from_global().expect("default store");

    let job = Job::new("global-smoke", "s1");
    let job_id = job.job_id.clone();
    installed.upsert_job(&job).await.unwrap();

    let fetched = resolved.get_job(&job_id).await.unwrap().expect("job");
    assert_eq!(fetched.job_name, "global-smoke");
}
