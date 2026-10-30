use super::paint::Painter;
use super::pool::{BUDGET, Tex, bytes};
use super::state::Sim;
use super::{Op, SceneView, Src};
use wprs_scene::{EffectDraw, MAX_STATEFUL};

const TICKS: f32 = 4.0;

type Held = (usize, (u32, u32), u32);

impl SceneView {
    pub fn state_bytes(&self) -> u64 {
        self.states.values().flat_map(|s| &s.tex).map(Tex::bytes).sum()
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn state(&mut self, i: usize, e: &EffectDraw<'_>, size: (u32, u32)) -> Option<Held> {
        let id = std::ptr::from_ref(e.effect) as usize;
        if let Err(why) = self.sim(id, size, e.reset) {
            self.leave(i, e, why);
            return None;
        }
        Some((id, size, self.ticks(id, self.frame.time)))
    }

    fn sim(&mut self, key: usize, size: (u32, u32), reset: u32) -> Result<(), String> {
        let (device, format, frame) = (&self.r.device, self.state_format, self.pool.frame);
        if self.states.get(&key).is_none_or(|s| s.tex[0].size() != size) {
            let others = self.states.iter().filter(|(k, _)| **k != key);
            let held: u64 = others.clone().flat_map(|(_, s)| &s.tex).map(Tex::bytes).sum();
            if others.count() >= MAX_STATEFUL {
                return Err(format!("the scene has more than {MAX_STATEFUL} effects that keep a state"));
            }
            if held + 2 * bytes(size, format) > BUDGET {
                return Err(format!("its state textures would take the scene past {} MiB", BUDGET >> 20));
            }
            let old = self.states.remove(&key).into_iter().flat_map(|s| s.tex);
            old.for_each(|t| self.pool.give(t));
            let tex = [self.pool.take(device, size, format), self.pool.take(device, size, format)];
            self.states.insert(key, Sim { tex, front: 0, used: frame, clock: None, owed: 0.0, reset });
            self.ops.extend([0, 1].map(|side| Op::Clear(Src::State(key, side), [0.0; 4])));
        }
        if let Some(sim) = self.states.get_mut(&key).filter(|s| s.reset != reset) {
            sim.reset = reset;
            self.ops.extend([0, 1].map(|side| Op::Clear(Src::State(key, side), [0.0; 4])));
        }
        if let Some(sim) = self.states.get_mut(&key) {
            sim.used = frame;
        }
        Ok(())
    }

    pub(crate) fn front(&self, key: usize) -> usize {
        self.states.get(&key).map_or(0, |s| s.front)
    }

    fn ticks(&mut self, key: usize, time: f32) -> u32 {
        let Some(sim) = self.states.get_mut(&key) else { return 0 };
        let owed = sim.clock.map_or(1.0, |c| sim.owed + (time - c).max(0.0) * 60.0);
        let n = (owed + 1e-3).floor().min(TICKS);
        (sim.clock, sim.owed) = (Some(time), if owed > TICKS { 0.0 } else { (owed - n).max(0.0) });
        n as u32
    }

    pub(crate) fn flip(&mut self, key: usize) {
        if let Some(sim) = self.states.get_mut(&key) {
            sim.front = 1 - sim.front;
        }
    }
}
