use super::Runtime;
use crate::eval::Frame;
use crate::puppet::{Puppet, Seat, Sim, figure};
use crate::resolve::Context;
use crate::{Content, Course, Quality, RigControl, VideoControl, Wake};
use std::collections::BTreeMap;

pub(crate) fn cast(scene: &crate::Scene) -> Vec<Puppet> {
    let rig = |l: &crate::Layer| match &l.kind {
        crate::Kind::Mesh(m) => scene.rigs.get(&m.mesh).cloned(),
        crate::Kind::Model(m) => scene.solids.get(&m.model)?.rig.clone(),
        _ => None,
    };
    let rigged = scene.layers().filter_map(|l| Puppet::new(l, rig(l)?));
    rigged.take(64).collect()
}

impl Runtime {
    pub fn puppet(&self, layer: &str) -> Option<&Puppet> {
        self.puppets.iter().find(|p| p.layer == layer)
    }

    pub(crate) fn recast(&mut self) {
        let mut puppets = std::mem::take(&mut self.puppets);
        puppets.retain(|p| self.layer(&p.layer).and_then(figure).is_some());
        let mut stack: Vec<&crate::Layer> = self.spawned().collect();
        while let Some(l) = stack.pop() {
            stack.extend(l.children());
            let crate::Kind::Mesh(m) = &l.kind else { continue };
            if puppets.len() >= 64 || puppets.iter().any(|p| p.layer == l.id) {
                continue;
            }
            puppets.extend(self.scene.rigs.get(&m.mesh).and_then(|rig| Puppet::new(l, rig.clone())));
        }
        self.puppets = puppets;
    }

    pub(crate) fn step_puppets(&mut self, dt: f32) {
        let mut puppets = std::mem::take(&mut self.puppets);
        let (mut ended, mut heard) = (std::mem::take(&mut self.ended), Vec::new());
        {
            let cx = self.context();
            for p in &mut puppets {
                let Some(m) = self.layer(&p.layer).and_then(figure) else { continue };
                cx.wake.set(Wake::Idle);
                (p.lag, p.since) = ((p.lag + dt).min(1.0), (p.since + dt).min(1.0));
                p.step(&cx, &m, dt, (&mut ended, &mut heard));
                p.wake = p.wake.min(cx.wake.get());
            }
        }
        (self.puppets, self.ended) = (puppets, ended);
        self.spread(heard);
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
            let h = crate::puppet::STEP;
            let gust = self.scene.canvas.gusts.map_or([0.65, 0.35], |g| [1.0 - g, g]);
            let frozen = self.level == Quality::Q0;
            let sim = Sim { frozen, h, wind: [x, -y], gust, t: self.t as f32, unit: 1.0 };
            let pointer = self.pointer.pos.filter(|_| self.uses().cursor);
            for (i, p) in puppets.iter_mut().enumerate() {
                let shown = i >= 64 || seen & (1 << i) != 0;
                let Some(layer) = self.layer(&p.layer) else { continue };
                let Some(m) = figure(layer) else { continue };
                if !(shown || p.dirty || p.posed.generation == 0) {
                    continue;
                }
                cx.wake.set(Wake::Idle);
                let placed =
                    m.unit == 1.0 && (!p.chains.is_empty() || self.script.rigs.contains_key(&p.layer));
                let seat = Seat::new(&cx, layer, &m, pointer.filter(|_| placed), placed);
                p.pose(&cx, &m, (&seat, &sim));
                (p.wake, p.dirty) = (p.wake.min(cx.wake.get()), false);
            }
        }
        self.puppets = puppets;
    }

    pub(crate) fn steer_rigs(&mut self, rigs: &BTreeMap<String, RigControl>) {
        let h = crate::puppet::STEP;
        for (id, set) in rigs.iter().filter(|(_, r)| !r.impulses.is_empty()) {
            self.puppets.iter_mut().filter(|p| p.layer == *id).for_each(|p| p.push(set, h));
        }
    }

    pub(crate) fn steer_puppets(&mut self, videos: &BTreeMap<String, VideoControl>) {
        let mut puppets = std::mem::take(&mut self.puppets);
        for (id, control) in videos {
            let Some((layer, clip)) = id.split_once('#') else { continue };
            let Some(m) = self.layer(layer).and_then(figure) else { continue };
            if let Some(p) = puppets.iter_mut().find(|p| p.layer == layer) {
                p.steer(&m, clip, &control.commands);
            }
        }
        self.puppets = puppets;
    }

    pub(crate) fn puppet_courses(&self, out: &mut Vec<Course>) {
        for p in &self.puppets {
            if let Some(m) = self.layer(&p.layer).and_then(figure) {
                p.courses(&m, out);
            }
        }
    }

    pub(crate) fn sight_puppets(&self, frame: &Frame<'_>) {
        let mut seen = 0u64;
        let mut see = |layer: &str| {
            let at = self.puppets.iter().take(64).position(|p| p.layer == layer);
            seen |= at.map_or(0, |i| 1 << i);
        };
        for d in frame.draws.iter().filter(|d| !d.hidden) {
            match &d.content {
                Content::Mesh { layer, .. } => see(layer),
                Content::Model { stage, .. } => {
                    let models = frame.stages.get(*stage).map_or(0..0, |s| s.models.clone());
                    frame.placed[models].iter().for_each(|p| see(&p.layer.id));
                }
                _ => {}
            }
        }
        self.seen.set(seen);
    }
}

impl Runtime {
    pub(crate) fn bones_read(&self, cx: &Context, layer: &crate::Layer) -> Vec<[f32; 6]> {
        let asked = self.script.watch.iter().any(|w| w.strip_suffix("#bones") == Some(layer.id.as_str()));
        let found = figure(layer).zip(self.puppet(&layer.id)).filter(|_| asked);
        found.map_or_else(Vec::new, |(m, p)| p.bone_frames(crate::puppet::mount::size(cx, layer, &m)))
    }
}
