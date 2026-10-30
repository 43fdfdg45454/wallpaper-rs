mod aim;
mod bake;
mod bound;
mod census;
mod checks;
mod children;
mod cloud;
mod collide;
mod contact;
mod control;
mod drawn;
mod drive;
mod emit;
mod feed;
mod field;
mod flock;
mod forces;
mod hatch;
mod instance;
mod lineage;
mod look;
mod motion;
mod ops;
mod pace;
mod pack;
mod place;
mod plan;
mod points;
mod pulls;
mod rules;
mod seed;
mod shapes;
mod spawn;
mod spread;
mod stack;
mod starts;
mod steer;
mod strokes;
mod swarm;
mod sweeps;
mod tone;
mod tune;
mod wave;

pub use bake::{Bake, CPU_PARTICLES, MAX_POINTS, MAX_TRAIL};
pub use census::{Census, mark};
pub use children::{Child, Inheritable, MAX_CHILDREN, Moment};
pub use cloud::{Cloud, Particle};
pub use collide::{Collider, Surface};
pub use control::{ControlPoint, Handle, MAX_CONTROL, Periodic, Perspective, Placement};
pub use emit::{Burst, Cue, Emitter, Follow, Init, MAX_STOPS};
pub use feed::MAX_EMIT;
pub use field::MAX_FIELD;
pub(crate) use flock::flock_of;
pub use flock::{Flock, MAX_DEPTH, MAX_PARTS, Part, flocks};
pub use forces::{Falloff, Force, Target};
pub use instance::{Instance, Tunable};
pub use look::{Frames, Orient, Refract, Render, Stack};
pub use ops::Operator;
pub use pack::{EVENT, MAX_HATCH, PARTICLE_EVENTS, Sim};
pub(crate) use plan::plan;
pub use seed::{
    Carry, Hsv, MAX_PALETTE, MAX_REMAPS, OffsetNoise, Palette, REMAP_INPUTS, Remap, RemapOp, RemapOutput,
    Turbulent, Wave,
};
pub use shapes::{Area, EmitShape, Refresh, Source};
pub use spread::{Num, Spread};
pub use stack::{bin, largest, stacked};
pub use steer::{Boids, KeepBetween, KeepDistance, Oscillate, Ring, SlowNear, Wobble};
pub use strokes::{Rope, Stretch, Trail};
pub use swarm::{Play, STEP, SUBSTEPS, Swarm, WARM, WARM_WORK};
pub use sweeps::{Around, Between, Bow, Sweep, Taper};
pub(crate) use tune::tuned;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_EMITTERS: usize = 4;
pub const MAX_FORCES: usize = 8;
pub const MAX_COLLIDERS: usize = 4;
pub const MAX_CURVE: usize = 64;
pub const MAX_PREWARM: f32 = 10.0;
pub const MAX_RATE: f32 = 1e5;
pub const MAX_EMITTERS2: usize = 8;
pub const MAX_FORCES2: usize = 16;
pub const MAX_COLLIDERS2: usize = 8;
pub const MAX_PREWARM2: f32 = 30.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct System {
    pub capacity: u32,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub prewarm: f32,
    pub emitters: Vec<Emitter>,
    #[serde(default)]
    pub init: Init,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forces: Vec<Operator>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collide: Vec<Collider>,
    #[serde(default)]
    pub render: Render,
    #[serde(default, skip_serializing_if = "is_local")]
    pub space: Placement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perspective: Option<Perspective>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<ControlPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Child>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lock: Vec<Tunable>,
}

impl System {
    pub fn listens(&self) -> (bool, bool, bool) {
        let cue = |c| self.emitters.iter().any(|e| e.burst.as_ref().is_some_and(|b| b.on == Some(c)));
        let pointer = cue(Cue::Click) || self.targets().any(|t| matches!(t, Target::Follow(_)));
        let pointer = pointer || self.emitters.iter().any(|e| e.follow.is_some());
        let pointer = pointer || self.points.iter().any(|p| p.follow.is_some());
        let gated = self.emitters.iter().any(|e| e.audio_gate);
        let heard = self.init.turbulent.is_some_and(|t| t.audio.is_some());
        let remaps = self.init.remap.iter().chain(self.forces.iter().filter_map(|f| match &f.force {
            Force::Remap(r) => Some(r),
            _ => None,
        }));
        let remaps: Vec<&str> = remaps.map(|r| r.input.as_str()).collect();
        let audio = gated || heard || remaps.iter().any(|i| i.starts_with("audio:"));
        let pointer = pointer || remaps.iter().any(|i| i.starts_with("cursor:"));
        (cue(Cue::Beat) || audio, pointer, cue(Cue::Click))
    }

    pub fn targets(&self) -> impl Iterator<Item = &Target> {
        let forces = self.forces.iter().flat_map(|f| match &f.force {
            Force::Attractor { target: t, .. }
            | Force::Vortex { center: t, .. }
            | Force::Orbit { center: t, .. }
            | Force::SlowNear(SlowNear { target: t, .. })
            | Force::KeepDistance(KeepDistance { target: t, .. }) => vec![t],
            Force::KeepBetween(k) => vec![&k.from, &k.to],
            _ => vec![],
        });
        let shapes = self.emitters.iter().flat_map(|e| match &e.shape {
            EmitShape::Around(a) => a.at.iter().collect(),
            EmitShape::Between(b) => vec![&b.from, &b.to],
            _ => vec![],
        });
        forces.chain(shapes).chain(self.init.inherit.iter().map(|c| &c.from))
    }

    pub fn overdraw(&self, canvas: [u32; 2]) -> f32 {
        let peak = self.render.size_over_life.iter().map(|k| k[1]).fold(0.0, f32::max);
        let peak = if self.render.size_over_life.is_empty() { 1.0 } else { peak };
        let side = (self.init.size[0] + self.init.size[1]) / 2.0 * peak;
        self.capacity as f32 * side * side / (canvas[0] as f32 * canvas[1] as f32).max(1.0)
    }
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_local(v: &Placement) -> bool {
    *v == Placement::Local
}
