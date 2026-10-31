use crate::acting::Acting;
use crate::extras::{self, Extras};
use crate::pack::Pack;
use crate::{props, saved};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use wprs_render::{GpuImage, Meter, SceneAssets, SceneView};
use wprs_scene::{FlashGuard, Governor, MAX_PRESSES, Press, Quality, Runtime, Spare};

static LEVEL: AtomicU32 = AtomicU32::new(0);
static CALMING: AtomicU32 = AtomicU32::new(0);
static ENGINE: AtomicU32 = AtomicU32::new(0);
static LEFT: Mutex<(u64, Vec<(String, String)>)> = Mutex::new((0, Vec::new()));
static RETRIES: AtomicU64 = AtomicU64::new(0);

type Guarded = (FlashGuard, Vec<f32>, i64, Option<Meter>);

mod render;
pub(crate) mod report;

pub(crate) struct Stage {
    pub(crate) uid: String,
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
    guard: Option<Box<Guarded>>,
    retried: u64,
    spare: Spare,
    governor: Governor,
    samples: u64,
    offset: f32,
    extras: Extras,
    assets: SceneAssets,
}

pub(crate) struct Shot<'a> {
    pub(crate) image: Option<&'a GpuImage>,
    pub(crate) fps: u32,
    pub(crate) delay: i64,
}

fn moves(id: &str) -> bool {
    crate::library::with(|lib| Ok(lib.decided(id, "motion")?)).unwrap_or(false)
}

impl Stage {
    pub(crate) fn new(uid: String, pack: Arc<Pack>) -> Stage {
        let mut runtime = Runtime::new(Arc::clone(&pack.scene), props::values(&pack));
        pack.inputs.into_iter().for_each(|allowed| runtime.declare(allowed));
        runtime.set_language(pack.locale.as_deref().unwrap_or_default());
        runtime.set_listeners(true, false);
        pack.assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
        let (granted, moving) = (props::granted(&pack.id), moves(&pack.id));
        let (view, quality, tick, due, touch) = (None, Quality::Q3, None, None, (None, 0, false));
        let acting = Acting::new(&pack, &runtime);
        let extras = Extras::gather(&pack, runtime.props(), extras::now(), &Extras::default());
        let assets = extras.assets(&pack);
        Stage {
            uid,
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
            guard: None,
            retried: RETRIES.load(Relaxed),
            spare: Spare::default(),
            governor: Governor::default(),
            samples: 0,
            offset: 0.5,
            extras,
            assets,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.granted = props::granted(&self.pack.id);
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

    pub(crate) fn videos(&self) -> Arc<Vec<(String, PathBuf)>> {
        Arc::clone(&self.extras.videos)
    }

    fn gather(&mut self) {
        let fresh = Extras::gather(&self.pack, self.runtime.props(), extras::now(), &self.extras);
        if !fresh.same(&self.extras) {
            (self.assets, self.restock, self.touch.2) = (fresh.assets(&self.pack), true, true);
        }
        self.extras = fresh;
    }

    pub(crate) fn turn(&mut self) {
        if self.extras.next.is_some_and(|at| extras::now() >= at) {
            self.gather();
        }
    }

    pub(crate) fn until_turn(&self, delay: i64) -> i64 {
        let Some(at) = self.extras.next else { return delay };
        let left = (at.saturating_sub(extras::now()) * 1000).max(1) as i64;
        if delay == 0 { left } else { delay.min(left) }
    }

    pub(crate) fn touch(&mut self, at: [f32; 2], down: bool) -> bool {
        let (moves, taps) = self.allowed();
        let buttons = u8::from(down);
        if taps && buttons != self.touch.1 && self.presses.len() < MAX_PRESSES {
            self.presses.push(Press { at: Some(at), buttons });
        }
        if moves || taps {
            self.touch = (down.then_some(at).filter(|_| moves), buttons, true);
        }
        moves || taps
    }

    pub(crate) fn offset(&mut self, x: f32) -> bool {
        let moved = self.offset != x && self.pack.scene.tilts();
        self.offset = x;
        self.touch.2 |= moved;
        moved
    }

    pub(crate) fn hide(&mut self) {
        self.tick = None;
        if let Some(view) = self.view.as_mut() {
            view.trim();
            self.runtime.restart_particles();
        }
    }

    pub(crate) fn drop_view(&mut self) {
        self.view = None;
    }

    pub(crate) fn rest(&mut self) {
        if self.due.is_none() {
            self.view.iter_mut().for_each(SceneView::trim);
        }
    }
}
