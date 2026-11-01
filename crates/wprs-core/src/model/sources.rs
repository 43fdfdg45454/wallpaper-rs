use super::{Time, Uid};
use std::path::PathBuf;

model! {
    pub struct Source {
        pub uid: Uid,
        pub path: PathBuf,
        pub recursive: bool,
        pub scanned_at: Option<Time>,
        pub error: Option<String>,
        pub items: u32,
        pub provider: Option<String>,
    }

    pub struct SourcesReply { pub model: u32, pub serial: u64, pub sources: Vec<Source> }

    pub struct Tag { pub name: String, pub count: u32 }

    pub struct TagsReply { pub model: u32, pub serial: u64, pub tags: Vec<Tag> }

    pub struct SourceScan { pub source: Option<Uid>, pub added: u32, pub updated: u32, pub moved: u32, pub missing: u32 }

    pub struct ScanOutput { pub reports: Vec<SourceScan> }

    pub struct Imported { pub path: PathBuf, pub uid: Uid }

    pub struct PathError { pub path: PathBuf, pub error: String }

    pub struct ImportReport { pub imported: Vec<Imported>, pub failed: Vec<PathError> }

    pub struct AnalyzeReport { pub analyzed: u32, pub failed: u32 }
}
