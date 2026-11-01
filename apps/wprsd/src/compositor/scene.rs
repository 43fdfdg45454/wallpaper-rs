mod level;
mod polling;
#[cfg(test)]
mod tests;

use super::SceneSpec;
use super::power::Power;
use super::scripts::Link;
use std::time::{Duration, Instant};
use wprs_core::Region;
use wprs_render::SceneView;
use wprs_scene::{
    Course, Fired, Governor, LayerRead, PointerState, Press, Quality, Runtime, Spare, Touch, Uses, Values,
    Voice, Why,
};

mod control;
mod feed;
mod frame;

const CALM: u32 = 30;
const SETTLE: u32 = 3;
const POLL: f64 = 0.5;

pub(super) struct SceneState {
    pub(super) spec: SceneSpec,
    runtime: Runtime,
    still: bool,
    reads: (bool, bool),
    quality: Quality,
    uses: Uses,
    view: Option<SceneView>,
    tick: Option<Instant>,
    pointer: PointerState,
    presses: Vec<Press>,
    allowed: [bool; 2],
    touches: Vec<Touch>,
    host: Option<Link>,
    calm: u32,
    answered: bool,
    awaiting: u64,
    timer: f64,
    sent: (Values, [f32; 4], [u32; 2], u32),
    read_back: Vec<(String, LayerRead)>,
    sounding: Vec<String>,
    pub(super) console: std::collections::VecDeque<String>,
    pub(super) error: Option<String>,
    pub(super) left: Vec<String>,
    left_key: u64,
    pub(super) due: Option<Instant>,
    pub(super) moving: bool,
    pub(super) voices: Vec<Voice>,
    pub(super) heard: Option<Instant>,
    exposed: Values,
    pressed: Vec<String>,
    fired: Vec<Fired>,
    courses: Vec<Course>,
    filmed: Vec<Course>,
    ended: Vec<String>,
    zone: (i64, String),
    restock: bool,
    pub(super) hushed: bool,
    pub(super) quiet: bool,
    guard: Option<Box<super::guard::Guard>>,
    pub(super) damage: Option<Region>,
    spare: Spare,
    governor: Governor,
    why: Why,
    samples: u64,
}

fn after(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds.clamp(0.0, 86_400.0)).unwrap_or_default()
}

impl SceneState {
    pub(super) fn new(spec: SceneSpec, power: Power) -> SceneState {
        let mut runtime = Runtime::new(spec.pack.scene.clone(), spec.values.clone());
        spec.pack.inputs.into_iter().for_each(|allowed| runtime.declare(allowed));
        runtime.set_language(&spec.pack.locale);
        spec.pack.assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
        runtime.set_lengths(spec.pack.lengths.clone());
        runtime.set_playing(spec.clips.iter().map(|(asset, _)| asset.clone()).collect());
        let still = spec.hold.is_some() || power.still(&runtime);
        match spec.hold {
            Some(time) => runtime.seek(time),
            None if still => {
                tracing::debug!("the scene is too heavy for this device and stays at its poster time");
                runtime.seek(f64::from(runtime.scene.clock.poster_time));
            }
            None => {}
        }
        let exposed = wprs_scene::exposed(&spec.pack.props.properties, &spec.values);
        let [audio, cursor] = wprs_scene::script_inputs(&spec.pack.scene, &spec.pack.scripts);
        let reads = spec.pack.inputs.map_or((audio, cursor), |u| (audio && u.audio, cursor && u.cursor));
        let (reads, (quality, why)) = (reads, power.level(&runtime));
        let (view, tick, pointer, calm, error, due, uses) = Default::default();
        let (moving, voices, heard) = (!still, Vec::new(), None);
        let mut scene = SceneState {
            spec,
            runtime,
            still,
            reads,
            quality,
            uses,
            view,
            tick,
            pointer,
            presses: Vec::new(),
            allowed: [false; 2],
            touches: Vec::new(),
            host: None,
            calm,
            answered: false,
            awaiting: 0,
            timer: f64::INFINITY,
            sent: (Values::new(), [0.0; 4], [1, 1], 60),
            read_back: Vec::new(),
            sounding: Vec::new(),
            console: Default::default(),
            error,
            left: Vec::new(),
            left_key: 0,
            due,
            moving,
            voices,
            heard,
            exposed,
            pressed: Vec::new(),
            fired: Vec::new(),
            courses: Vec::new(),
            filmed: Vec::new(),
            ended: Vec::new(),
            zone: (i64::MIN, String::new()),
            restock: false,
            hushed: false,
            quiet: false,
            guard: None,
            damage: None,
            spare: Spare::default(),
            governor: Governor::default(),
            why,
            samples: 0,
        };
        scene.uses = scene.needs(quality);
        scene
    }

    fn needs(&self, quality: Quality) -> Uses {
        if self.still {
            return Uses::default();
        }
        let uses = self.runtime.uses_at(quality);
        Uses { audio: uses.audio || self.reads.0, cursor: uses.cursor || self.reads.1, ..uses }
    }
}

fn assets(spec: &SceneSpec) -> wprs_render::SceneAssets {
    let mut assets = spec.pack.assets.clone();
    assets.images.extend(spec.extra.iter().cloned());
    assets
}
