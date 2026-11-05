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

impl MediaState {
    pub fn cues(&self, before: &MediaState) -> Vec<&'static str> {
        let on = |s: &MediaState| s.present() && s.playing;
        let fresh = self.present() && (!before.present() || self.track != before.track);
        let covered = self.enabled && self.cover != 0 && self.cover != before.cover;
        let found = [("play", on(self) && !on(before)), ("pause", on(before) && !on(self))];
        let found = found.into_iter().chain([("track", fresh), ("cover", covered)]);
        found.filter(|(_, fired)| *fired).map(|(name, _)| name).collect()
    }
}
