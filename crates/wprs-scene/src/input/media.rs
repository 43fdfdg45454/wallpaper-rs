use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaState {
    pub enabled: bool,
    pub track: u64,
    pub cover: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub subtitle: String,
    pub genres: String,
    pub kind: String,
    pub playing: bool,
    pub position: f32,
    pub duration: f32,
    pub rate: f32,
    pub colors: Option<[[f32; 4]; 5]>,
}

impl MediaState {
    pub fn present(&self) -> bool {
        self.enabled && self.track != 0
    }
}
