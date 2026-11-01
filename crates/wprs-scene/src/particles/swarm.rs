use super::{Flock, MAX_EMITTERS2, Part, Sim, System};

pub const STEP: f32 = 1.0 / 60.0;
pub const WARM: f32 = 1.0 / 15.0;
pub const SUBSTEPS: usize = 15;
pub const WARM_WORK: u64 = 30_000_000;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Feed {
    pub rate: f32,
    pub owed: f32,
    pub pending: u32,
    pub next: f64,
    pub flip: f64,
    pub off: bool,
    pub on: bool,
    pub window: u32,
    pub sequence: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Play {
    #[default]
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Default)]
pub struct Swarm {
    pub epoch: u32,
    pub stamp: u64,
    pub parts: usize,
    pub steps: Vec<Sim>,
    pub play: Play,
    pub(super) bases: Vec<Sim>,
    pub(super) frames: Vec<Sim>,
    pub(super) feeds: Vec<[Feed; MAX_EMITTERS2]>,
    pub(super) hosts: Vec<Option<usize>>,
    pub(super) clock: Option<f64>,
    pub(super) acc: f64,
    pub(super) sim: f64,
    pub(super) start: f64,
    pub(super) death: Vec<f64>,
    pub(super) beat: bool,
    pub(super) buttons: u8,
    pub(super) emits: u32,
    pub(super) scale: f64,
    pub(super) warm: usize,
    pub(super) places: Vec<crate::Affine>,
    pub(super) moves: Vec<[[f32; 6]; MAX_EMITTERS2]>,
}

impl Swarm {
    pub(crate) fn new(
        systems: &[System],
        flock: &Flock,
        (seed, work): (u64, u64),
        canvas: [u32; 2],
    ) -> Swarm {
        let host = |p: &Part| p.host.map(|h| &systems[flock.parts[h].system]);
        let bases: Vec<Sim> =
            flock.parts.iter().map(|p| Sim::of(&systems[p.system], p, host(p), seed, canvas)).collect();
        let n = bases.len();
        let hosts = flock.parts.iter().map(|p| p.host).collect();
        let warm = (WARM_WORK / work.max(1)).max(1) as usize;
        let mut swarm = Swarm { parts: n, frames: bases.clone(), bases, hosts, warm, ..Swarm::default() };
        swarm.restart();
        swarm.epoch = 0;
        swarm
    }

    pub(crate) fn restart(&mut self) {
        let epoch = self.epoch.wrapping_add(1);
        let mut bases = std::mem::take(&mut self.bases);
        bases.iter_mut().for_each(|b| (b.ids[1], b.ids[2]) = (0, 0));
        let (n, hosts, frames) =
            (bases.len(), std::mem::take(&mut self.hosts), std::mem::take(&mut self.frames));
        *self = Swarm {
            warm: self.warm,
            places: std::mem::take(&mut self.places),
            moves: std::mem::take(&mut self.moves),
            epoch,
            stamp: self.stamp,
            parts: n,
            bases,
            frames,
            hosts,
            feeds: vec![[Feed::default(); MAX_EMITTERS2]; n],
            death: vec![f64::NEG_INFINITY; n],
            scale: 1.0,
            ..Swarm::default()
        };
    }

    pub fn count(&self) -> usize {
        self.steps.len() / self.parts.max(1)
    }

    pub fn sim(&self, step: usize, part: usize) -> &Sim {
        &self.steps[step * self.parts + part]
    }
}
