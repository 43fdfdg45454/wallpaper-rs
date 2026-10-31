use crate::pages::PageSpec;
use crate::scenes::ScenePack;
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use wprs_core::{Edit, Fit, Place};
use wprs_render::DepthMap;
use wprs_scene::{AudioFrame, PointerState, ScriptOutputs, Values};
use wprs_video::Cut;

#[derive(Clone)]
pub enum Content {
    Color,
    Image(Picture),
    Video(Movie),
    Scene(SceneSpec),
    Web(PageSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reel {
    pub path: PathBuf,
    pub cut: Cut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Movie {
    pub reel: Reel,
    pub volume: u32,
    pub zoom: f32,
    pub pan: [f32; 2],
}

#[derive(Clone)]
pub struct SceneSpec {
    pub pack: Arc<ScenePack>,
    pub values: Values,
    pub extra: Vec<(String, Arc<wprs_render::Pixels>)>,
    pub clips: Vec<(String, Reel)>,
    pub hold: Option<f64>,
    pub moving: bool,
}

#[derive(Debug, Clone)]
pub enum SceneInput {
    Audio(String, Box<AudioFrame>),
    Pointer(String, PointerState, Vec<wprs_scene::Press>, [bool; 2]),
    Script(String, Box<ScriptOutputs>),
}

#[derive(Clone)]
pub struct Picture {
    pub file: PathBuf,
    pub modified: Option<SystemTime>,
    pub pixels: Option<Arc<RgbaImage>>,
    pub edit: Option<Arc<Edit>>,
    pub place: Place,
    pub depth: Option<Depth>,
}

#[derive(Clone)]
pub struct Depth {
    pub file: PathBuf,
    pub modified: Option<SystemTime>,
    pub map: Arc<DepthMap>,
}

impl Content {
    pub(crate) fn same_source(&self, other: &Content) -> bool {
        match (self, other) {
            (Content::Color, Content::Color) => true,
            (Content::Image(a), Content::Image(b)) => (&a.file, a.modified) == (&b.file, b.modified),
            (Content::Video(a), Content::Video(b)) => a.reel.path == b.reel.path,
            (Content::Scene(a), Content::Scene(b)) => Arc::ptr_eq(&a.pack, &b.pack),
            (Content::Web(a), Content::Web(b)) => a.pack.path == b.pack.path && a.privacy == b.privacy,
            _ => false,
        }
    }

    pub(crate) fn same_look(&self, other: &Content) -> bool {
        let look = |c: &Content| match c {
            Content::Image(p) => {
                let depth = p.depth.as_ref().map(|d| (d.file.clone(), d.modified));
                Some((p.edit.clone(), p.place.clone(), depth))
            }
            _ => None,
        };
        let values = |c: &Content| match c {
            Content::Scene(s) => Some((s.values.clone(), s.media(), s.hold, s.moving)),
            _ => None,
        };
        let frame = |c: &Content| match c {
            Content::Video(m) => Some((m.reel.cut, m.volume, m.zoom, m.pan)),
            _ => None,
        };
        let page = |c: &Content| match c {
            Content::Web(p) => Some(p.clone()),
            _ => None,
        };
        let same = look(self) == look(other) && values(self) == values(other) && frame(self) == frame(other);
        let same = same && page(self) == page(other);
        self.same_source(other) && same
    }

    pub(crate) fn page(&self) -> Option<&PageSpec> {
        match self {
            Content::Web(page) => Some(page),
            _ => None,
        }
    }

    pub(crate) fn video(&self) -> Option<&Reel> {
        match self {
            Content::Video(movie) => Some(&movie.reel),
            _ => None,
        }
    }

    pub(crate) fn videos(&self) -> Vec<(&Reel, u32)> {
        match self {
            Content::Video(movie) => vec![(&movie.reel, movie.volume)],
            Content::Scene(s) => s.reels().map(|(_, reel, volume)| (reel, volume)).collect(),
            _ => Vec::new(),
        }
    }
}

impl Reel {
    pub fn plain(path: PathBuf) -> Reel {
        Reel { path, cut: Cut::default() }
    }
}

pub(crate) fn loudness(volume: f64, own: u32) -> f64 {
    volume * f64::from(own.min(100)) / 100.0
}

impl SceneSpec {
    pub(crate) fn media(&self) -> Vec<(String, usize, Option<Reel>)> {
        let files = self.extra.iter().map(|(k, i)| (k.clone(), Arc::as_ptr(i) as usize, None));
        files.chain(self.clips.iter().map(|(k, reel)| (k.clone(), 0, Some(reel.clone())))).collect()
    }

    pub(crate) fn reels(&self) -> impl Iterator<Item = (&str, &Reel, u32)> {
        let replaced =
            |id: &String| self.clips.iter().any(|(k, _)| k == id) || self.extra.iter().any(|(k, _)| k == id);
        let own =
            self.pack.videos.iter().filter(move |(id, _)| !replaced(id)).map(|(id, r)| (id.as_str(), r, 100));
        own.chain(self.clips.iter().map(|(id, reel)| (id.as_str(), reel, 0)))
    }
}

#[derive(Clone)]
pub struct Spec {
    pub content: Content,
    pub fit: Fit,
    pub background: [u8; 3],
    pub transition: Option<Duration>,
    pub refresh: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub transition: Duration,
    pub max_fps: u32,
    pub volume: f64,
    pub muted: bool,
    pub software_parallax: bool,
    pub flash_guard: bool,
    pub scene_sound: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct OutputStatus {
    pub kind: &'static str,
    pub playing: bool,
    pub error: Option<String>,
    pub parallax: bool,
    pub frames: u64,
    pub fps: u32,
    pub sound: &'static str,
    pub flashes: &'static str,
    pub particles: &'static str,
    pub quality: &'static str,
    pub quality_reason: &'static str,
    pub decode: &'static str,
    pub left_out: Vec<String>,
    pub console: Vec<String>,
    pub sandbox: &'static str,
}
