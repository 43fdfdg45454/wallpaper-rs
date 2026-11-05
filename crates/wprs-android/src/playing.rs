use image::imageops::FilterType;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use wprs_render::Pixels;
use wprs_scene::{COVER_SIDE, FALLBACK, MediaState, PALETTE_SIDE, palette};

const MAX_TEXT: usize = 512;
const MAX_SIDE: u32 = 4096;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Told {
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    genres: String,
    playing: bool,
    position: f64,
    duration: f64,
    rate: Option<f64>,
    keep_cover: bool,
}

pub(crate) struct Art {
    pub(crate) serial: u64,
    pub(crate) pixels: Arc<Pixels>,
    pub(crate) colors: [[f32; 4]; 5],
    digest: u64,
}

pub(crate) struct Played {
    pub(crate) serial: u64,
    pub(crate) state: MediaState,
    read: Instant,
    pub(crate) cover: Option<Arc<Art>>,
    pub(crate) previous: Option<Arc<Art>>,
}

static NOW: Mutex<Option<Arc<Played>>> = Mutex::new(None);
static SERIAL: AtomicU64 = AtomicU64::new(1);
static ACCESS: AtomicBool = AtomicBool::new(false);

fn lock() -> MutexGuard<'static, Option<Arc<Played>>> {
    NOW.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Played {
    pub(crate) fn at(&self, now: Instant) -> MediaState {
        let mut state = self.state.clone();
        if state.playing {
            let moved = state.position + now.saturating_duration_since(self.read).as_secs_f32() * state.rate;
            state.position =
                if state.duration > 0.0 { moved.clamp(0.0, state.duration) } else { moved.max(0.0) };
        }
        state
    }
}

fn text(s: String) -> String {
    s.chars().filter(|c| !c.is_control()).take(MAX_TEXT).collect()
}

fn seconds(v: f64) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1e7) as f32 } else { 0.0 }
}

pub(crate) fn colors_of(image: &image::RgbaImage) -> Option<[[f32; 4]; 5]> {
    palette(image::imageops::resize(image, PALETTE_SIDE, PALETTE_SIDE, FilterType::Triangle).as_raw())
}

fn art(argb: &[i32], width: u32) -> Option<Arc<Art>> {
    let height = u32::try_from(argb.len().checked_div(width as usize)?).ok()?;
    let fits = width > 0 && height > 0 && width <= MAX_SIDE && height <= MAX_SIDE;
    if !fits || argb.len() != (width * height) as usize {
        return None;
    }
    let rgba: Vec<u8> = argb.iter().flat_map(|c| c.rotate_left(8).to_be_bytes()).collect();
    let digest = fnv(&rgba);
    let mut image = image::RgbaImage::from_raw(width, height, rgba)?;
    if width.max(height) > COVER_SIDE {
        let scale = f64::from(COVER_SIDE) / f64::from(width.max(height));
        let side = |v: u32| ((f64::from(v) * scale).round() as u32).clamp(1, COVER_SIDE);
        image = image::imageops::resize(&image, side(width), side(height), FilterType::Triangle);
    }
    let colors = colors_of(&image).unwrap_or(FALLBACK);
    let serial = SERIAL.fetch_add(1, Relaxed);
    Some(Arc::new(Art { serial, pixels: Pixels::new(image), colors, digest }))
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

pub(crate) fn heard(told: Option<&str>, argb: Option<&[i32]>, width: u32) {
    let mut now = lock();
    let Some(t) = told.and_then(|t| serde_json::from_str::<Told>(t).ok()) else { return *now = None };
    let before = now.clone();
    let last = before.as_ref().and_then(|b| b.cover.clone());
    let earlier = before.as_ref().and_then(|b| b.previous.clone());
    let fresh = argb.and_then(|a| art(a, width));
    let (cover, previous) = match fresh {
        _ if t.keep_cover => (last, earlier),
        Some(f) if last.as_ref().is_some_and(|l| l.digest == f.digest) => (last, earlier),
        fresh => (fresh, last.or(earlier)),
    };
    let song = [text(t.title), text(t.artist), text(t.album), text(t.album_artist)];
    let same = before.as_ref().is_some_and(|b| {
        let s = &b.state;
        [&s.title, &s.artist, &s.album, &s.album_artist] == [&song[0], &song[1], &song[2], &song[3]]
    });
    let track = before.as_ref().map_or(0, |b| b.state.track) + u64::from(!same);
    let [title, artist, album, album_artist] = song;
    let rate = t.rate.filter(|r| r.is_finite()).map_or(1.0, |r| r.clamp(-100.0, 100.0) as f32);
    let state = MediaState {
        enabled: true,
        track,
        cover: cover.as_ref().map_or(0, |c| c.serial),
        title,
        artist,
        album,
        album_artist,
        genres: text(t.genres),
        kind: "music".into(),
        playing: t.playing,
        position: seconds(t.position),
        duration: seconds(t.duration),
        rate,
        colors: cover.as_ref().map(|c| c.colors),
        ..MediaState::default()
    };
    let serial = SERIAL.fetch_add(1, Relaxed);
    *now = Some(Arc::new(Played { serial, state, read: Instant::now(), cover, previous }));
}

pub(crate) fn now() -> Option<Arc<Played>> {
    lock().clone()
}

pub(crate) fn access() -> bool {
    ACCESS.load(Relaxed)
}

pub(crate) fn allow(on: bool) {
    ACCESS.store(on, Relaxed);
}
