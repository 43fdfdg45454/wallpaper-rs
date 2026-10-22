mod api;
mod doc;
mod engine;
mod enroll;
mod events;
mod hlc;
mod keys;
mod order;
mod plan;
mod record;
mod session;
mod share;
#[cfg(test)]
mod tests;

pub use api::{AGENT, Api, check_url};
pub use doc::{Doc, View};
pub use engine::{Change, Key, Outcome, Page, Remote, Replica, sync};
pub use enroll::{Poll, poll};
pub use events::Parser;
pub use hlc::{Clock, Hlc, now_ms};
pub use keys::{Ids, SyncKey};
pub use order::positions;
pub use plan::{Rules, Step, step};
pub use record::{Context, open, pad, seal, unpad};
pub use session::Session;
pub use share::{DeviceKey, code, open_key, seal_key};

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

pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the system random source failed");
    bytes
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
