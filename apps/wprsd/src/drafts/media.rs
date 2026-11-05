#[cfg(test)]
mod tests;

use crate::error::invalid;
use crate::media::{Frame, covers};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Component, Path};
use wprs_scene::MediaState;

const MAX_TEXT: usize = 512;
const MAX_SECONDS: f32 = 1e7;

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum Words {
    #[default]
    None,
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Simulated {
    title: String,
    artist: Words,
    album: String,
    album_artist: Words,
    subtitle: String,
    genres: Words,
    playing: bool,
    position: f32,
    duration: f32,
    rate: Option<f32>,
    cover: Option<String>,
}

fn text(name: &str, words: Words) -> Result<String> {
    let joined = match words {
        Words::None => String::new(),
        Words::One(one) => one,
        Words::Many(many) => many.join(", "),
    };
    let fits = joined.chars().count() <= MAX_TEXT && !joined.chars().any(char::is_control);
    ensure!(
        fits,
        invalid(format!("media.{name} holds at most {MAX_TEXT} characters and no control characters"))
    );
    Ok(joined)
}

fn seconds(name: &str, value: f32) -> Result<f32> {
    let fits = value.is_finite() && (0.0..=MAX_SECONDS).contains(&value);
    ensure!(fits, invalid(format!("media.{name} goes from 0 to {MAX_SECONDS} seconds, not {value}")));
    Ok(value)
}

fn inside(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

pub(crate) fn split(document: &str) -> (String, Option<Value>) {
    if !document.contains("\"media\"") {
        return (document.to_owned(), None);
    }
    let Ok(mut doc) = serde_json::from_str::<Value>(document) else { return (document.to_owned(), None) };
    let media = doc["preview"].as_object_mut().and_then(|p| p.remove("media"));
    match media {
        Some(media) => (doc.to_string(), Some(media)),
        None => (document.to_owned(), None),
    }
}

pub(crate) fn frame(dir: &Path, media: Value) -> Result<Frame> {
    let s: Simulated =
        serde_json::from_value(media).map_err(|e| invalid(format!("the simulated media player: {e}")))?;
    let rate = s.rate.unwrap_or(1.0);
    ensure!(
        rate.is_finite() && (-100.0..=100.0).contains(&rate),
        invalid("media.rate goes from -100 to 100")
    );
    let duration = seconds("duration", s.duration)?;
    let position = seconds("position", s.position)?;
    let cover = match s.cover.filter(|c| !c.is_empty()) {
        None => None,
        Some(path) => {
            ensure!(inside(&path), invalid("media.cover is a picture inside the scene folder"));
            let bytes = std::fs::read(dir.join(&path)).ok().filter(|b| b.len() as u64 <= covers::MAX_FILE);
            let decoded = bytes.and_then(|b| covers::decode(&b));
            Some(
                decoded.ok_or_else(|| {
                    invalid(format!("media.cover {path} is not a picture that can be read"))
                })?,
            )
        }
    };
    let state = MediaState {
        enabled: true,
        track: 1,
        cover: cover.as_ref().map_or(0, |c| c.serial),
        title: text("title", Words::One(s.title))?,
        artist: text("artist", s.artist)?,
        album: text("album", Words::One(s.album))?,
        album_artist: text("album_artist", s.album_artist)?,
        subtitle: text("subtitle", Words::One(s.subtitle))?,
        genres: text("genres", s.genres)?,
        kind: "music".into(),
        playing: s.playing,
        position: if duration > 0.0 { position.min(duration) } else { position },
        duration,
        rate,
        colors: None,
    };
    Ok(Frame { cover, ..Frame::new(state) })
}
