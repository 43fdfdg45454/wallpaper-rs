use std::collections::HashMap;
use zbus::{OwnedVariant, Variant};

const MAX_TEXT: usize = 512;
const MAX_LIST: usize = 16;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Track {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artists: Vec<String>,
    pub(crate) album: String,
    pub(crate) album_artists: Vec<String>,
    pub(crate) genres: Vec<String>,
    pub(crate) length: f64,
    pub(crate) art: String,
}

fn text(v: &Variant) -> Option<String> {
    let found = match v {
        Variant::Variant(inner) => return text(inner),
        Variant::ObjectPath(p) => p.as_str().to_owned(),
        other => String::try_from(other).ok()?,
    };
    Some(found.chars().filter(|c| !c.is_control()).take(MAX_TEXT).collect())
}

fn texts(v: &Variant) -> Vec<String> {
    match v {
        Variant::Variant(inner) => texts(inner),
        Variant::Array(a) => a.iter().filter_map(text).filter(|t| !t.is_empty()).take(MAX_LIST).collect(),
        other => text(other).into_iter().filter(|t| !t.is_empty()).collect(),
    }
}

pub(super) fn number(v: &Variant) -> Option<f64> {
    let n = match v {
        Variant::Variant(inner) => return number(inner),
        Variant::I64(n) => *n as f64,
        Variant::U64(n) => *n as f64,
        Variant::I32(n) => f64::from(*n),
        Variant::U32(n) => f64::from(*n),
        Variant::F64(n) => *n,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

pub(super) fn word(v: &Variant) -> Option<String> {
    text(v)
}

pub(super) fn dict(v: &Variant) -> HashMap<String, OwnedVariant> {
    match v {
        Variant::Variant(inner) => dict(inner),
        other => other.try_clone().ok().and_then(|v| HashMap::try_from(v).ok()).unwrap_or_default(),
    }
}

impl Track {
    pub(crate) fn read(data: &HashMap<String, OwnedVariant>) -> Track {
        let one = |key: &str| data.get(key).and_then(|v| text(v)).unwrap_or_default();
        let many = |key: &str| data.get(key).map(|v| texts(v)).unwrap_or_default();
        let length = data.get("mpris:length").and_then(|v| number(v)).unwrap_or(0.0);
        Track {
            id: one("mpris:trackid"),
            title: one("xesam:title"),
            artists: many("xesam:artist"),
            album: one("xesam:album"),
            album_artists: many("xesam:albumArtist"),
            genres: many("xesam:genre"),
            length: (length / 1e6).clamp(0.0, 1e7),
            art: one("mpris:artUrl"),
        }
    }

    pub(crate) fn known(&self) -> bool {
        !self.title.is_empty() || !self.artists.is_empty() || !self.id.is_empty()
    }

    pub(crate) fn same_song(&self, other: &Track) -> bool {
        (&self.id, &self.title, &self.artists, &self.album)
            == (&other.id, &other.title, &other.artists, &other.album)
    }

    pub(crate) fn fill(&self, state: &mut wprs_scene::MediaState) {
        state.title = self.title.clone();
        state.artist = self.artists.join(", ");
        state.album = self.album.clone();
        state.album_artist = self.album_artists.join(", ");
        state.subtitle = String::new();
        state.genres = self.genres.join(", ");
        state.kind = if self.known() { "music".into() } else { String::new() };
        state.duration = self.length as f32;
    }
}
