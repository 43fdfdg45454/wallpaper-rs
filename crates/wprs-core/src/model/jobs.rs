use super::Time;
use serde_json::Value;

model! {
    pub struct Job {
        pub id: u64,
        pub kind: JobKind,
        pub target: String,
        pub state: JobState,
        pub done: u64,
        pub total: u64,
        pub started_at: Time,
        pub finished_at: Option<Time>,
        pub error: Option<JobError>,
        pub output: Option<Value>,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum JobKind {
        Scan, Import, Analyze, ProviderSync, RepoAdd, RepoRefresh, Install, Account, Link, AccountSync, Upload,
        #[serde(other)] Unknown,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum JobState { Queued, Running, Done, Failed, Cancelled, #[serde(other)] Unknown }

    pub struct JobError { pub name: String, pub message: String }

    pub struct JobsReply { pub model: u32, pub serial: u64, pub jobs: Vec<Job> }

    pub struct JobReply { pub model: u32, pub serial: u64, pub job: Job }
}
