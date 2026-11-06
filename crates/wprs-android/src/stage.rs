use crate::acting::Acting;
use crate::extras::{self, Extras};
use crate::pack::Pack;
use crate::{props, saved};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use wprs_render::{GpuImage, Meter, SceneAssets, SceneView};
use wprs_scene::{FlashGuard, Governor, Press, Quality, Runtime, Spare};

mod heard;
mod input;
#[cfg(test)]
mod probe;

pub(crate) use heard::allowed as media_allowed;

static CALMING: AtomicU32 = AtomicU32::new(0);
static ENGINE: AtomicU32 = AtomicU32::new(0);
static RETRIES: AtomicU64 = AtomicU64::new(0);

type Guarded = (FlashGuard, Vec<f32>, i64, Option<Meter>);

mod render;
pub(crate) mod report;

pub(crate) struct Stage {
    pub(crate) uid: String,
    pub(crate) lock: bool,
    pub(crate) pack: Arc<Pack>,
    runtime: Runtime,
    view: Option<SceneView>,
    granted: [bool; 3],
    acting: Acting,
    presses: Vec<Press>,
    rect: [f32; 4],
    posed: bool,
    heavy: bool,
    quality: Quality,
    fps: u32,
    tick: Option<i64>,
    due: Option<i64>,
    touch: (Option<[f32; 2]>, u8, bool),
    restock: bool,
    calmed: bool,
    stilled: bool,
    pub(crate) moving: bool,
    pub(crate) reports: Option<usize>,
    guard: Option<Box<Guarded>>,
    retried: u64,
    spare: Spare,
    governor: Governor,
    samples: u64,
    offset: f32,
    extras: Extras,
    assets: SceneAssets,
    media: heard::Shown,
    gate: crate::opened::Gate,
    opens: Vec<serde_json::Value>,
}

pub(crate) struct Shot<'a> {
    pub(crate) image: Option<&'a GpuImage>,
    pub(crate) fps: u32,
    pub(crate) delay: i64,
}

fn moves(id: &str) -> bool {
    crate::library::with(|lib| Ok(lib.moves(id)?)).unwrap_or(false)
}

impl Stage {
    pub(crate) fn light(&mut self, headroom: f32) {
        self.view.iter_mut().for_each(|view| view.light(headroom));
    }

    pub(crate) fn top(&self) -> bool {
        self.quality == Quality::Q3 && self.pack.scene.canvas.bright()
    }

    #[cfg(test)]
    pub(crate) fn new(uid: String, pack: Arc<Pack>) -> Stage {
        Stage::on(uid, pack, (false, false))
    }

    pub(crate) fn on(uid: String, pack: Arc<Pack>, (lock, preview): (bool, bool)) -> Stage {
        let mut runtime = Runtime::new(Arc::clone(&pack.scene), props::values(&pack));
        pack.inputs.into_iter().for_each(|allowed| runtime.declare(allowed));
        runtime.set_leveling(wprs_scene::Leveling::of(&pack.properties));
        runtime.set_language(pack.locale.as_deref().unwrap_or_default());
        runtime.set_listeners(true, false);
        pack.assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
        let films =
            pack.videos.iter().filter_map(|(id, path)| Some((id.clone(), wprs_scene::film_file(path)?)));
        runtime.set_lengths(films.collect());
        let (granted, moving) = (props::granted(&pack.id), moves(&pack.id));
        let media = heard::Shown::new(heard::allowed(&pack));
        let (view, quality, tick, due, touch) = (None, Quality::Q3, None, None, (None, 0, false));
        let acting = Acting::new(&pack, &runtime, (lock, preview));
        let extras = Extras::gather(&pack, (runtime.props(), &[]), extras::now(), &Extras::default());
        let assets = extras.assets(&pack);
        Stage {
            uid,
            lock,
            pack,
            runtime,
            view,
            granted,
            acting,
            presses: Vec::new(),
            rect: [0.0; 4],
            posed: false,
            heavy: false,
            quality,
            fps: 0,
            tick,
            due,
            touch,
            restock: false,
            calmed: false,
            stilled: false,
            moving,
            reports: Some(report::HOME),
            guard: None,
            retried: RETRIES.load(Relaxed),
            spare: Spare::default(),
            governor: Governor::default(),
            samples: 0,
            offset: 0.5,
            extras,
            assets,
            media,
            gate: Default::default(),
            opens: Vec::new(),
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.granted = props::granted(&self.pack.id);
        self.media.granted = heard::allowed(&self.pack);
        self.moving = moves(&self.pack.id);
        let changed = self.runtime.set_props(props::values(&self.pack));
        if changed {
            self.acting.reprops();
        }
        self.restock |= changed;
        self.gather();
        let pressed = saved::pressed(&self.pack.id);
        pressed.iter().for_each(|key| self.runtime.press(key));
        self.acting.actions.extend(pressed.iter().cloned());
        self.touch.2 |= changed || !pressed.is_empty();
    }

    #[cfg(test)]
    pub(crate) fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub(crate) fn wants(&self) -> [bool; 2] {
        let uses = self.runtime.uses_at(self.quality);
        let (moves, taps) = self.allowed();
        [uses.audio && self.granted[0], moves || taps]
    }

    fn allowed(&self) -> (bool, bool) {
        let uses = self.runtime.uses_at(self.quality);
        (uses.cursor && self.granted[1], uses.clicks && self.granted[2])
    }

    pub(crate) fn plays(&self) -> bool {
        !self.heavy && !self.extras.videos.is_empty()
    }

    pub(crate) fn image(&self) -> Option<&GpuImage> {
        self.view.as_ref()?.image()
    }

    pub(crate) fn tapes(&self) -> &[wprs_scene::Tape] {
        self.runtime.tapes()
    }

    pub(crate) fn videos(&self) -> Arc<Vec<(String, PathBuf)>> {
        Arc::clone(&self.extras.videos)
    }

    pub(crate) fn gather(&mut self) {
        let shown = &self.acting.keeping.shown;
        let fresh = Extras::gather(&self.pack, (self.runtime.props(), shown), extras::now(), &self.extras);
        if !fresh.same(&self.extras) {
            (self.assets, self.restock, self.touch.2) = (self.stocked(&fresh), true, true);
        }
        let user = |(k, _): &&(String, PathBuf)| !self.pack.videos.iter().any(|(o, _)| o == k);
        let new = |(k, _): &&(String, PathBuf)| !self.extras.videos.iter().any(|(o, _)| o == k);
        let films = fresh.videos.iter().filter(user).filter(new).cloned().collect();
        let mut fresh = fresh;
        fresh.lengths = std::mem::take(&mut self.extras.lengths);
        fresh.measure(films);
        self.extras = fresh;
    }
}
