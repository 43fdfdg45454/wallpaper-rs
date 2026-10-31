use super::Runtime;
use crate::input::{AudioFrame, PointerState};
use crate::particles::{Flock, Swarm};
use crate::uses::Uses;
use crate::{Quality, Values, Wake};
use std::cell::Ref;
use std::sync::Arc;

impl Runtime {
    pub fn declare(&mut self, allowed: Uses) {
        self.limit = Some(allowed);
    }

    pub fn uses(&self) -> Uses {
        self.uses_at(Quality::Q3)
    }

    pub fn uses_at(&self, quality: Quality) -> Uses {
        let found = self.uses[quality as usize];
        let Some(limit) = self.limit else { return found };
        let (audio, cursor, clicks) =
            (found.audio && limit.audio, found.cursor && limit.cursor, found.clicks && limit.clicks);
        Uses { audio, cursor, clicks, media: found.media && limit.media, ..found }
    }

    pub fn since_beat(&self) -> f32 {
        (self.t - self.beat).min(f64::from(f32::MAX)) as f32
    }

    pub fn time(&self) -> f64 {
        self.t
    }

    pub fn props(&self) -> &Values {
        &self.props
    }

    pub fn audio(&self) -> &AudioFrame {
        &self.audio
    }

    pub fn pointer(&self) -> &PointerState {
        &self.pointer
    }

    pub fn dt(&self) -> f32 {
        self.dt
    }

    pub fn cost(&self, quality: Quality) -> f32 {
        let at = Arc::as_ptr(&self.scene) as usize;
        let (kept, mut costs) = self.costs.get();
        costs = if kept == at { costs } else { [None; 4] };
        let cost = *costs[quality as usize].get_or_insert_with(|| {
            self.scene.cost(quality) + self.scene.copies_cost(self.spawned(), quality)
        });
        self.costs.set((at, costs));
        cost
    }

    pub fn calmed(&self, wake: Wake) -> Wake {
        let minute = 60.0 - self.wall.rem_euclid(60.0);
        match wake {
            Wake::Frame => Wake::At(minute),
            Wake::At(at) => Wake::At(at.max(minute)),
            Wake::Idle => Wake::Idle,
        }
    }

    pub fn pace(&self) -> u32 {
        self.pace.get().max(1)
    }

    pub fn wake(&self) -> Wake {
        self.wake.get()
    }

    pub fn flocks(&self) -> &[Flock] {
        &self.flocks
    }

    pub fn swarm(&self, system: usize) -> Option<Ref<'_, Swarm>> {
        Ref::filter_map(self.swarms.borrow(), |s| s.get(system)).ok()
    }
}
