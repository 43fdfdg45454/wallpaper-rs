macro_rules! model {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        $item
    )*};
}

mod account;
mod inputs;
mod jobs;
mod library;
mod playlists;
mod remote;
mod schema;
mod sources;
mod state;

pub use account::*;
pub use inputs::*;
pub use jobs::*;
pub use library::*;
pub use playlists::*;
pub use remote::*;
pub use schema::*;
pub use sources::*;
pub use state::*;

pub const MODEL: u32 = 1;
pub type Uid = String;
pub type Time = i64;

model! {
    pub struct Header { pub model: u32, pub serial: u64 }

    #[derive(Copy, Eq, Hash)]
    pub enum Topic { State, Library, Playlists, Profiles, Providers, Repos, Jobs, Inputs, Account, #[serde(other)] Unknown }
}
