macro_rules! model {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        $item
    )*};
}

mod library;
mod playlists;
mod remote;
mod state;

pub use library::*;
pub use playlists::*;
pub use remote::*;
pub use state::*;

pub const MODEL: u32 = 1;
pub type Uid = String;
pub type Time = i64;

model! {
    pub struct Header { pub model: u32, pub serial: u64 }

    #[derive(Copy, Eq, Hash)]
    pub enum Topic { State, Library, Playlists, Profiles, Providers, Repos, Jobs, #[serde(other)] Unknown }
}
