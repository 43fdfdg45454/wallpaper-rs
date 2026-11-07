use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoPace {
    #[default]
    Detail,
    Smooth,
}

impl AutoPace {
    pub const WORDS: [&str; 2] = ["detail", "smooth"];
    pub const KEY: &str = "playback.auto_pace";

    pub fn as_str(self) -> &'static str {
        Self::WORDS[self as usize]
    }

    pub fn halves(self) -> bool {
        self == AutoPace::Detail
    }
}
