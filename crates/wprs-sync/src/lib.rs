mod api;
mod blob;
mod doc;
mod engine;
mod enroll;
mod escrow;
mod events;
mod gone;
mod hlc;
mod kept;
mod keys;
mod mailbox;
mod order;
mod plan;
mod record;
mod session;
mod share;
#[cfg(test)]
mod tests;
mod vault;
mod views;

pub use api::{AGENT, Api, MAIL, PAGE, SMALL, check_url};
pub use blob::{SEALED, SEGMENT, open_segment, seal_segment, sealed_size};
pub use doc::{Doc, View};
pub use engine::{Change, Key, LIFETIME_MS, Outcome, Page, Remote, Replica, Synced, rekey, sync};
pub use enroll::{Poll, poll};
pub use escrow::{Cost, Lock, MIN_PASSPHRASE, STRONG, escrow_epoch, unseal};
pub use events::Parser;
pub use gone::{deleted_before, removals, removed_blob, removed_package};
pub use hlc::{Clock, Hlc, now_ms};
pub use kept::{arrived, hold, keep, keep_name, playlist_rules, renamed, rules, text};
pub use keys::{Ids, SyncKey};
pub use mailbox::{Letter, Peer, ack, devices, from, inbox, next, nonce, post};
pub use order::{clash, positions};
pub use plan::{Clash, Rules, Step, Winner, step};
pub use record::{Context, open, pad, seal, unpad};
pub use session::Session;
pub use share::{DeviceKey, Mail, aad, code, commit, open_key, seal_key, transcript};
pub use vault::{Secrets, encode};
pub use views::{
    DEVICE_KEYS, DOWNLOADS, Kept, Local, REFERENCES, edit_fields, edit_held, edit_of, item_view, kind, link,
    object, preserve, references, settings,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Crypto(&'static str),
    #[error("{detail}")]
    Server { status: u16, code: String, detail: String },
    #[error("cannot reach the server: {0}")]
    Network(String),
    #[error("{0}")]
    Invalid(String),
}

impl Error {
    pub fn code(&self) -> &str {
        match self {
            Error::Server { code, .. } => code,
            _ => "",
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Invalid(format!("unexpected data: {e}"))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub const FORMAT: i64 = 1;
pub const TOO_OLD: &str = "client_too_old";

pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the system random source failed");
    bytes
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
