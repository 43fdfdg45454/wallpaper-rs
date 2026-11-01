use super::Runtime;
use crate::eval::Frame;
use crate::puppet::{Puppet, Seat, Sim, mesh};
use crate::resolve::Context;
use crate::{Content, Course, Quality, RigControl, VideoControl, Wake};
use std::collections::BTreeMap;

pub(crate) fn cast(scene: &crate::Scene) -> Vec<Puppet> {
    let rigged = scene.layers().filter_map(|l| Puppet::new(l, scene.rigs.get(&mesh(l)?.mesh)?.clone()));
    rigged.take(64).collect()
}

impl Runtime {
    pub fn puppet(&self, layer: &str) -> Option<&Puppet> {
        self.puppets.iter().find(|p| p.layer == layer)
    }

    pub(crate) fn step_puppets(&mut self, dt: f32) {
        let mut puppets = std::mem::take(&mut self.puppets);
        let mut ended = std::mem::take(&mut self.ended);
        {
            let cx = self.context();
            for p in &mut puppets {
                let Some(m) = self.scene.layer(&p.layer).and_then(mesh) else { continue };
                cx.wake.set(Wake::Idle);
                p.lag = (p.lag + dt).min(1.0);
                p.step(&cx, m, dt, &mut ended);
                p.wake = p.wake.min(cx.wake.get());
            }
        }
        (self.puppets, self.ended) = (puppets, ended);
    }

    pub(crate) fn pose_puppets(&mut self) {
        let mut puppets = std::mem::take(&mut self.puppets);
        let seen = self.seen.get();
        let wind = self.scene.canvas.wind.as_ref();
        {
            let cx = self.context();
            let camera = cx.lens(self.level);
            let cx = Context { view: self.viewed.get(), camera, ..cx };
            let [x, y] = wind.map_or([0.0; 2], |w| w.get(&cx).0);
            let h = if self.battery { 1.0 / 30.0 } else { 1.0 / 60.0 };
            let sim = Sim { frozen: self.level == Quality::Q0, h, wind: [x, -y], t: self.t as f32 };
            let pointer = self.pointer.pos.filter(|_| self.uses().cursor);
            for (i, p) in puppets.iter_mut().enumerate() {
                let shown = i >= 64 || seen & (1 << i) != 0;
                let Some(layer) = self.scene.layer(&p.layer) else { continue };
                let Some(m) = mesh(layer) else { continue };
                if !(shown || p.dirty || p.posed.generation == 0) {
                    continue;
                }
                cx.wake.set(Wake::Idle);
                let placed = !p.chains.is_empty() || self.script.rigs.contains_key(&p.layer);
                let seat = Seat::new(&cx, layer, m, pointer.filter(|_| placed), placed);
                p.pose(&cx, m, (&seat, &sim));
                (p.wake, p.dirty) = (p.wake.min(cx.wake.get()), false);
            }
        }
        self.puppets = puppets;
    }

    pub(crate) fn steer_rigs(&mut self, rigs: &BTreeMap<String, RigControl>) {
        let h = if self.battery { 1.0 / 30.0 } else { 1.0 / 60.0 };
        for (id, set) in rigs.iter().filter(|(_, r)| !r.impulses.is_empty()) {
            self.puppets.iter_mut().filter(|p| p.layer == *id).for_each(|p| p.push(set, h));
        }
    }

    pub(crate) fn steer_puppets(&mut self, videos: &BTreeMap<String, VideoControl>) {
        for (id, control) in videos {
            let Some((layer, clip)) = id.split_once('#') else { continue };
            let Some(m) = self.scene.layer(layer).and_then(mesh) else { continue };
            if let Some(p) = self.puppets.iter_mut().find(|p| p.layer == layer) {
                p.steer(m, clip, &control.commands);
            }
        }
    }

    pub(crate) fn puppet_courses(&self, out: &mut Vec<Course>) {
        for p in &self.puppets {
            if let Some(m) = self.scene.layer(&p.layer).and_then(mesh) {
                p.courses(m, out);
            }
        }
    }

    pub(crate) fn sight_puppets(&self, frame: &Frame<'_>) {
        let mut seen = 0u64;
        for d in frame.draws.iter().filter(|d| !d.hidden) {
            if let Content::Mesh { layer, .. } = &d.content {
                let at = self.puppets.iter().take(64).position(|p| p.layer == *layer);
                seen |= at.map_or(0, |i| 1 << i);
            }
        }
        self.seen.set(seen);
    }
}

impl Runtime {
    pub(crate) fn bones_read(&self, cx: &Context, layer: &crate::Layer) -> Vec<[f32; 6]> {
        let asked = self.script.watch.iter().any(|w| w.strip_suffix("#bones") == Some(layer.id.as_str()));
        let found = mesh(layer).zip(self.puppet(&layer.id)).filter(|_| asked);
        found.map_or_else(Vec::new, |(m, p)| p.bone_frames(crate::puppet::mount::size(cx, layer, m)))
    }
}
