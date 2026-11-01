use crate::cue::Fired;
use crate::input::{AudioFrame, PointerState, ScriptOutputs};
use crate::particles::{Flock, Swarm, System, flocks};
use crate::timeline::Playhead;
use crate::uses::{self, Uses};
use crate::{Layer, Scene, Timeline, Values, Wake};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

mod adopted;
mod clock;
mod draw;
mod feed;
mod puppets;
mod reads;

use draw::holds;

const MAX_STEP: f64 = 0.1;
const MAX_GAP: f64 = 600.0;

pub struct Runtime {
    pub scene: Arc<Scene>,
    pub seed: u64,
    props: Values,
    raw: Values,
    leveling: crate::Leveling,
    level: crate::Quality,
    audio: AudioFrame,
    heard: f64,
    pointer: PointerState,
    glide: PointerState,
    glided: f64,
    tilt: [f32; 2],
    pub(crate) still: bool,
    homes: HashMap<String, String>,
    sheets: BTreeMap<String, Vec<f32>>,
    pub(crate) script: ScriptOutputs,
    pub(crate) spawned: Vec<(String, Box<Layer>)>,
    pub(crate) pointing: crate::pointing::Pointing,
    pub(crate) played: crate::played::Played,
    wall: f64,
    lang: String,
    t: f64,
    dt: f32,
    stirred: [f64; 2],
    beat: f64,
    pub(crate) timelines: Vec<(Timeline, Playhead)>,
    pub(crate) fired: Vec<Fired>,
    pub(crate) listened: BTreeSet<String>,
    pub(crate) onsets: Vec<Option<f64>>,
    pub(crate) gates: RefCell<Vec<Option<f64>>>,
    pub(crate) lists: Vec<crate::Playlist>,
    mattes: BTreeSet<String>,
    playing: BTreeSet<String>,
    memo: RefCell<HashMap<usize, [f32; 4]>>,
    places: RefCell<HashMap<usize, crate::kin::Place>>,
    motion: RefCell<HashMap<usize, crate::resolve::Motion>>,
    wake: Cell<Wake>,
    pace: Cell<u32>,
    sizes: Cell<(usize, usize)>,
    pub(crate) costs: Cell<(usize, [Option<f32>; 4])>,
    uses: [Uses; 4],
    limit: Option<Uses>,
    swarms: RefCell<Vec<Swarm>>,
    flocks: Vec<Flock>,
    systems: Vec<System>,
    glow: crate::glow::Glows,
    battery: bool,
    pub(crate) tapes: Vec<crate::Tape>,
    pub(crate) ended: Vec<String>,
    pub(crate) lengths: BTreeMap<String, f32>,
    pub(crate) sighted: Cell<u64>,
    pub(crate) puppets: Vec<crate::puppet::Puppet>,
    pub(crate) seen: Cell<u64>,
}

impl Runtime {
    pub fn new(scene: Arc<Scene>, props: Values) -> Runtime {
        let timelines = scene.timelines.iter().map(|tl| (tl.clone(), tl.start())).collect();
        let uses = uses::of(&scene, &props);
        let targets = scene.layers().flat_map(|l| {
            let masks = l.mask.iter().chain(l.effects.iter().filter_map(|e| e.mask.as_ref()));
            let fed =
                l.effects.iter().flat_map(|e| e.params.values()).filter_map(|v| v.binding()?.layer.clone());
            masks.filter_map(|m| m.layer.clone()).chain(fed)
        });
        let targets = targets.collect();
        let mut mattes = BTreeSet::new();
        holds(&scene.layers, &targets, &mut mattes);
        let (audio, pointer, script, memo) = Default::default();
        let wake = Cell::new(Wake::Frame);
        let fresh = || std::time::UNIX_EPOCH.elapsed().map_or(0, |d| d.as_nanos() as u64 ^ d.as_secs() << 32);
        let (seed, size) = (scene.clock.seed.unwrap_or_else(fresh), scene.canvas.size);
        let homes = crate::kin::homes(&scene);
        let flocks = flocks(&scene);
        let systems: Vec<System> = scene.particles.values().cloned().collect();
        let work = flocks.iter().flat_map(|f| &f.parts).map(|p| u64::from(systems[p.system].capacity)).sum();
        let swarms =
            RefCell::new(flocks.iter().map(|f| Swarm::new(&systems, f, (seed, work), size)).collect());
        let listened = crate::voice::listened(&scene, true, true);
        let onsets = crate::voice::onsets(&scene);
        let gates = RefCell::new(vec![None; onsets.len()]);
        let glow = crate::glow::glows(&scene.canvas);
        let tapes = crate::footage::tapes(&scene);
        let mut runtime = Runtime {
            glow,
            battery: false,
            tapes,
            ended: Vec::new(),
            lengths: BTreeMap::new(),
            sighted: Cell::new(u64::MAX),
            puppets: puppets::cast(&scene),
            seen: Cell::new(u64::MAX),
            scene,
            seed,
            raw: props.clone(),
            leveling: Default::default(),
            level: Default::default(),
            props,
            audio,
            heard: 0.0,
            pointer,
            glide: pointer,
            glided: 0.0,
            tilt: [0.0; 2],
            still: false,
            homes,
            sheets: BTreeMap::new(),
            script,
            spawned: Vec::new(),
            pointing: Default::default(),
            played: Default::default(),
            wall: 0.0,
            lang: String::new(),
            t: 0.0,
            dt: 0.0,
            stirred: [f64::NEG_INFINITY; 2],
            beat: f64::NEG_INFINITY,
            timelines,
            fired: Vec::new(),
            listened,
            onsets,
            gates,
            lists: Vec::new(),
            mattes,
            playing: BTreeSet::new(),
            memo,
            places: RefCell::default(),
            motion: RefCell::default(),
            wake,
            pace: Cell::new(u32::MAX),
            sizes: Cell::default(),
            costs: Cell::default(),
            uses,
            limit: None,
            swarms,
            flocks,
            systems,
        };
        runtime.set_lengths(BTreeMap::new());
        runtime.step_puppets(0.0);
        runtime.pose_puppets();
        runtime
    }
}
