use super::{Instance, MAX_CHILDREN, Moment, Placement};
use crate::{Kind, Scene, ScriptApi};

pub const MAX_PARTS: usize = 64;
pub const MAX_DEPTH: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub system: usize,
    pub host: Option<usize>,
    pub link: usize,
    pub on: Option<Moment>,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Flock {
    pub system: usize,
    pub layer: Option<String>,
    pub parts: Vec<Part>,
}

impl Instance {
    pub fn moves(&self) -> bool {
        let set = [&self.count, &self.speed, &self.life, &self.time_scale];
        set.iter().any(|v| v.is_some()) || !self.points.is_empty()
    }
}

pub fn flocks(scene: &Scene) -> Vec<Flock> {
    let scripted = scene.scripts.iter().any(|s| s.wants(ScriptApi::Particles));
    let mut out = Vec::new();
    for (i, (id, s)) in scene.particles.iter().enumerate() {
        let (mut shared, mut own) = (false, Vec::new());
        for l in scene.layers() {
            let Kind::Particles { system, instance, .. } = &l.kind else { continue };
            if system != id {
                continue;
            }
            match scripted || s.space == Placement::World || instance.as_ref().is_some_and(|i| i.moves()) {
                true => own.push(l.id.clone()),
                false => shared = true,
            }
        }
        let tree = parts(scene, i);
        out.extend(shared.then(|| Flock { system: i, layer: None, parts: tree.clone() }));
        out.extend(own.into_iter().map(|layer| Flock { system: i, layer: Some(layer), parts: tree.clone() }));
    }
    out
}

pub(crate) fn flock_of(flocks: &[Flock], system: usize, layer: &str) -> usize {
    let own = flocks.iter().position(|f| f.system == system && f.layer.as_deref() == Some(layer));
    own.or_else(|| flocks.iter().position(|f| f.system == system && f.layer.is_none())).unwrap_or(usize::MAX)
}

fn parts(scene: &Scene, root: usize) -> Vec<Part> {
    let systems: Vec<_> = scene.particles.values().collect();
    let ids: Vec<&String> = scene.particles.keys().collect();
    let mut parts = vec![Part { system: root, host: None, link: 0, on: None, depth: 0 }];
    let mut k = 0;
    while k < parts.len() {
        let part = parts[k].clone();
        let mut line = vec![part.system];
        let mut at = part.host;
        while let Some(a) = at {
            line.push(parts[a].system);
            at = parts[a].host;
        }
        for (link, c) in systems[part.system].children.iter().enumerate().take(MAX_CHILDREN) {
            let Some(j) = ids.iter().position(|id| **id == c.system) else { continue };
            if part.depth < MAX_DEPTH && parts.len() < MAX_PARTS && !line.contains(&j) {
                let on = Some(c.on);
                parts.push(Part { system: j, host: Some(k), link, on, depth: part.depth + 1 });
            }
        }
        k += 1;
    }
    parts
}
