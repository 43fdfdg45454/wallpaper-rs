use super::{Time, Uid};

model! {
    #[derive(Copy, Eq, Hash)]
    pub enum AccountState { Unlinked, Linking, NeedsKey, Ready, Paused, Error, #[serde(other)] Unknown }

    pub struct AccountDevice {
        pub id: String,
        pub name: String,
        pub label: Option<String>,
        pub platform: String,
        pub created_at: Time,
        pub seen_at: Option<Time>,
        pub current: bool,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum Recovery { Off, Current, Stale, #[serde(other)] Unknown }

    pub struct RepoOffer {
        pub id: String,
        pub repo_id: String,
        pub name: String,
        pub url: String,
        pub fingerprints: Vec<String>,
    }

    pub struct Pairing {
        pub device: String,
        pub code: Option<String>,
        pub sharing: bool,
    }

    pub struct Account {
        pub url: String,
        pub name: String,
        pub repo_id: Option<String>,
        pub handle: Option<String>,
        pub device: Option<String>,
        pub label: String,
        pub state: AccountState,
        pub encrypted: Option<bool>,
        pub warning: Option<String>,
        pub paused: bool,
        pub last_sync: Option<Time>,
        pub last_error: Option<String>,
        pub records: u32,
        pub used_bytes: u64,
        pub quota_bytes: u64,
        pub devices: Vec<AccountDevice>,
        pub pairing: Option<Pairing>,
        pub quarantined: u32,
        pub unapplied: u32,
        pub offers: Vec<RepoOffer>,
        pub secrets: Vec<String>,
        pub recovery: Recovery,
        pub files_bytes: u64,
        pub files_quota_bytes: u64,
        pub job: Option<u64>,
    }

    pub struct AccountReply { pub model: u32, pub serial: u64, pub account: Option<Account> }

    pub struct PrivateFile {
        pub id: String,
        pub name: String,
        pub size: u64,
        pub item: Option<Uid>,
        pub shared: bool,
        pub stored: bool,
    }

    pub struct FilesReply { pub model: u32, pub serial: u64, pub folders: Vec<String>, pub files: Vec<PrivateFile> }

    pub struct UploadOutput {
        pub id: String,
        pub state: String,
        pub package: Option<String>,
        pub version: Option<String>,
        pub errors: Vec<String>,
    }
}
