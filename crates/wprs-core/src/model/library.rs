use super::{InstalledPackage, Preset, Reason, Time, Uid};
use std::path::PathBuf;

model! {
    #[derive(Copy, Eq, Hash)]
    pub enum ItemKind { Image, Video, Package, #[serde(other)] Unknown }

    pub struct PackageInfo {
        pub id: String,
        pub version: String,
        pub kind: String,
        pub license: String,
        #[serde(default)]
        pub content: Option<String>,
        #[serde(default)]
        pub photosensitive: Option<bool>,
        #[serde(default)]
        pub facets: Vec<String>,
        #[serde(default)]
        pub confirmed: Vec<String>,
    }

    pub struct About {
        pub schema: String,
        pub title: String,
        pub description: String,
        pub changes: String,
        pub authors: Vec<String>,
        pub license: String,
        pub content: Option<String>,
        pub descriptors: Vec<String>,
        pub photosensitive: Option<bool>,
        pub origin: Option<String>,
        pub derived_from: Vec<Work>,
        pub credits: Vec<PackageCredit>,
        pub links: Vec<Link>,
        pub inputs: Vec<String>,
        pub features: Vec<Feature>,
        pub languages: Vec<String>,
        #[serde(default)]
        pub provider: Option<String>,
        #[serde(default)]
        pub source_url: Option<String>,
        #[serde(default)]
        pub platforms: Vec<String>,
        #[serde(default)]
        pub packages: Vec<String>,
        #[serde(default)]
        pub refused: Vec<String>,
        #[serde(default)]
        pub animated: Option<String>,
    }

    pub struct Work { pub author: String, pub title: Option<String>, pub url: Option<String>, pub license: Option<String> }

    pub struct PackageCredit {
        pub paths: Vec<String>,
        pub license: String,
        pub attribution: String,
        pub url: Option<String>,
        #[serde(default)]
        pub source: Option<String>,
    }

    pub struct Link { pub kind: String, pub url: String }

    pub struct Feature { pub name: String, pub why: String }

    pub struct Item {
        pub uid: Uid,
        pub source: Option<Uid>,
        pub kind: ItemKind,
        pub path: PathBuf,
        pub title: String,
        pub size: u64,
        pub mtime: Time,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub duration_ms: Option<u32>,
        pub audio: Option<bool>,
        pub color: Option<u32>,
        pub error: Option<String>,
        pub package: Option<PackageInfo>,
        pub tags: Vec<String>,
        pub rating: u8,
        pub favorite: bool,
        pub added_at: Time,
        pub used_at: Option<Time>,
        pub use_count: u32,
        pub missing: bool,
    }

    pub struct ItemSummary {
        pub uid: Uid,
        pub kind: ItemKind,
        pub title: String,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub duration_ms: Option<u32>,
        pub color: Option<u32>,
        pub tags: Vec<String>,
        pub rating: u8,
        pub favorite: bool,
        pub missing: bool,
        pub error: Option<String>,
        pub revoked: Option<Reason>,
        pub removable: bool,
        pub imported: bool,
        pub thumb: Option<PathBuf>,
        #[serde(default)]
        pub package: Option<PackageInfo>,
    }

    pub struct SearchPage {
        pub model: u32,
        pub serial: u64,
        pub query: String,
        pub total: u32,
        pub offset: u32,
        pub items: Vec<ItemSummary>,
    }

    pub struct ItemDetails {
        pub model: u32,
        pub serial: u64,
        pub item: Item,
        pub thumb: Option<PathBuf>,
        pub presets: Vec<Preset>,
        pub playlists: Vec<PlaylistRef>,
        pub credit: Option<Credit>,
        pub installed: Option<InstalledPackage>,
        pub revoked: Option<Reason>,
        #[serde(default)]
        pub about: Option<About>,
    }

    pub struct PlaylistRef { pub uid: Uid, pub name: String }

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

    pub struct Credit {
        pub provider: String,
        pub title: String,
        pub attribution: Option<String>,
        pub license: Option<String>,
        pub link: Option<String>,
    }
}

pub fn asks(what: &str, content: Option<&str>, photosensitive: Option<bool>, confirmed: &[String]) -> bool {
    let flagged = if what == "mature" { content == Some("mature") } else { photosensitive == Some(true) };
    flagged && !confirmed.iter().any(|c| c == what)
}

impl PackageInfo {
    pub fn asks(&self, what: &str) -> bool {
        asks(what, self.content.as_deref(), self.photosensitive, &self.confirmed)
    }
}
