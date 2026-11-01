use super::{ItemSummary, Time, Uid};
use crate::Fit;
use std::collections::BTreeMap;
use std::path::PathBuf;

model! {
    #[derive(Copy, Eq, Hash)]
    pub enum Order { Sequential, Shuffle, #[serde(other)] Unknown }

    pub struct Rules {
        pub order: Order,
        pub interval: String,
        pub at: Vec<String>,
        pub days: Vec<String>,
        pub synced: bool,
        pub resume: bool,
        pub advance_while_paused: bool,
        pub skip_videos_on_battery: bool,
        pub transition_ms: Option<u32>,
    }

    pub struct Playlist {
        pub uid: Uid,
        pub name: String,
        pub query: Option<String>,
        pub rules: Rules,
        pub count: u32,
        pub outputs: Vec<String>,
    }

    pub struct PlaylistsReply { pub model: u32, pub serial: u64, pub playlists: Vec<Playlist> }

    pub struct PlaylistPage {
        pub model: u32,
        pub serial: u64,
        pub playlist: Playlist,
        pub total: u32,
        pub offset: u32,
        pub items: Vec<ItemSummary>,
    }

    pub struct HistoryEntry { pub at: Time, pub item: Option<ItemSummary> }

    pub struct HistoryReply { pub model: u32, pub serial: u64, pub output: String, pub entries: Vec<HistoryEntry> }

    pub struct ProfileOutput {
        pub item: Option<Uid>,
        pub path: Option<PathBuf>,
        pub playlist: Option<Uid>,
        pub fit: Option<Fit>,
        pub background: Option<String>,
        pub preset: Option<Uid>,
        pub filter: Option<crate::Filter>,
    }

    pub struct ProfileAuto { pub outputs: Option<Vec<String>>, pub on_battery: Option<bool> }

    pub struct Profile {
        pub uid: Uid,
        pub name: String,
        pub outputs: BTreeMap<String, ProfileOutput>,
        pub options: BTreeMap<String, String>,
        pub auto: ProfileAuto,
        pub priority: i32,
        pub active: bool,
    }

    pub struct ProfilesReply { pub model: u32, pub serial: u64, pub active: Option<Uid>, pub profiles: Vec<Profile> }

    pub struct ProfileReply { pub model: u32, pub serial: u64, pub profile: Profile }

    pub struct PresetValue { pub fit: Option<Fit>, pub background: Option<String> }

    pub struct Preset { pub uid: Uid, pub item: Option<Uid>, pub name: String, pub value: PresetValue }

    pub struct PresetsReply { pub model: u32, pub serial: u64, pub item: Option<Uid>, pub presets: Vec<Preset> }
}
