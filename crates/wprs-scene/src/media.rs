use crate::probe::Probe;
use crate::props::Text;
use crate::validate::{Check, Issue};
use crate::value::Binding;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const COVER_SIDE: u32 = 256;
pub const MEDIA_TRIGGERS: [&str; 4] = ["play", "pause", "track", "cover"];
pub const MAX_PLACEHOLDER: usize = 512;

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

fn language(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let first = parts.next().unwrap_or_default();
    let base = (2..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_alphabetic());
    base && parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

impl Check<'_> {
    pub(crate) fn placeholder<T: Probe>(&mut self, path: &str, b: &Binding<T>) {
        let Some(text) = &b.placeholder else { return };
        let p = format!("{path}/placeholder");
        if !T::TEXT || !b.media.is_some_and(|m| m.reading() == Reading::Words) {
            self.issues.push(Issue::Type(p.clone()));
        }
        let long = |s: &str| s.chars().count() as f32;
        match text {
            Text::Plain(s) => self.range(&p, long(s), 0.0, MAX_PLACEHOLDER as f32),
            Text::Localized(map) if map.is_empty() => self.issues.push(Issue::Empty(p)),
            Text::Localized(map) => {
                for (tag, s) in map {
                    let q = format!("{p}/{tag}");
                    if !language(tag) {
                        self.issues.push(Issue::Id(q.clone()));
                    }
                    self.range(&q, long(s), 0.0, MAX_PLACEHOLDER as f32);
                }
            }
        }
    }
}
