use super::*;

fn entry(id: u64, kind: JobKind) -> Entry {
    let (target, state) = (String::new(), JobState::Running);
    let (finished_at, error, output) = (None, None, None);
    let job = Job { id, kind, target, state, done: 0, total: 0, started_at: 0, finished_at, error, output };
    Entry { job, cancel: Arc::default() }
}

#[tokio::test(flavor = "multi_thread")]
async fn waiting_covers_only_the_given_jobs() {
    let jobs = Arc::new(Jobs::new());
    lock(&jobs.list).extend([entry(1, JobKind::RepoRefresh), entry(2, JobKind::Install)]);
    let _turn = jobs.repos.acquire().await.unwrap();
    assert!(!jobs.scanning());
    let waiter = tokio::task::spawn_blocking({
        let jobs = jobs.clone();
        move || jobs.wait(&[2])
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!waiter.is_finished());
    jobs.with(2, |e| e.job.state = JobState::Cancelled);
    tokio::time::timeout(Duration::from_secs(2), waiter).await.unwrap().unwrap();
    lock(&jobs.list).push(entry(3, JobKind::Analyze));
    assert!(active(&jobs.list()[0]) && jobs.scanning());
}

#[test]
fn a_change_during_a_running_scan_gets_a_scan_of_its_own() {
    let (running, mut queued) = (entry(1, JobKind::Scan).job, entry(2, JobKind::Scan).job);
    queued.state = JobState::Queued;
    assert!(!joins(&running, JobKind::Scan, "") && joins(&queued, JobKind::Scan, ""));
    assert!(!joins(&queued, JobKind::Scan, "other") && !joins(&queued, JobKind::Import, ""));
    let install = entry(3, JobKind::Install).job;
    assert!(joins(&install, JobKind::Install, "") && !joins(&install, JobKind::Analyze, ""));
}
