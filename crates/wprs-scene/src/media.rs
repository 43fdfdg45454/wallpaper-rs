use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MEDIA_TRIGGERS: [&str; 4] = ["play", "pause", "track", "cover"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Media {
    Title,
    Artist,
    Album,
    AlbumArtist,
    Subtitle,
    Genres,
    Kind,
    Playing,
    Position,
    Duration,
    Progress,
    Primary,
    Secondary,
    Tertiary,
    Text,
    Contrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cover {
    Current,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Words,
    Number,
    Flag,
    Color,
}

impl Media {
    pub fn reading(self) -> Reading {
        match self {
            Media::Title
            | Media::Artist
            | Media::Album
            | Media::AlbumArtist
            | Media::Subtitle
            | Media::Genres
            | Media::Kind => Reading::Words,
            Media::Playing => Reading::Flag,
            Media::Position | Media::Duration | Media::Progress => Reading::Number,
            Media::Primary | Media::Secondary | Media::Tertiary | Media::Text | Media::Contrast => {
                Reading::Color
            }
        }
    }
}
