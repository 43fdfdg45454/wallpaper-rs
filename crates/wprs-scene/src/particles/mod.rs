mod bake;
mod cloud;
mod emit;
mod forces;
mod look;
mod motion;
mod pack;
mod plan;
mod rules;
mod spawn;
mod swarm;

pub use bake::{Bake, CPU_PARTICLES, MAX_POINTS};
pub use cloud::{Census, Cloud, Particle};
pub use emit::{Burst, Cue, EmitShape, Emitter, Follow, Init};
pub use forces::{Collider, Force, Surface, Target};
pub use look::{Frames, Render, Trail};
pub use pack::Sim;
pub(crate) use plan::plan;
pub use swarm::{STEP, SUBSTEPS, Swarm};

use serde::{Deserialize, Serialize};

pub const MAX_EMITTERS: usize = 4;
pub const MAX_FORCES: usize = 8;
pub const MAX_COLLIDERS: usize = 4;
pub const MAX_CURVE: usize = 64;
pub const MAX_PREWARM: f32 = 10.0;
pub const MAX_RATE: f32 = 1e5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub forces: Vec<Force>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collide: Vec<Collider>,
    #[serde(default)]
    pub render: Render,
}

impl System {
    pub fn listens(&self) -> (bool, bool, bool) {
        let cue = |c| self.emitters.iter().any(|e| e.burst.as_ref().is_some_and(|b| b.on == Some(c)));
        let target = |t: &Target| matches!(t, Target::Follow(_));
        let follows = self.forces.iter().any(|f| match f {
            Force::Attractor { target: t, .. }
            | Force::Vortex { center: t, .. }
            | Force::Orbit { center: t, .. } => target(t),
            _ => false,
        });
        let pointer = cue(Cue::Click) || follows || self.emitters.iter().any(|e| e.follow.is_some());
        (cue(Cue::Beat), pointer, cue(Cue::Click))
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
