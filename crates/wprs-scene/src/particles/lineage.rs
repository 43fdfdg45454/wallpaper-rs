use super::{MAX_CHILDREN, MAX_CONTROL, Moment, Placement, System};
use crate::validate::{Check, Issue};
use crate::{MAX_PARTICLES, Scene};

impl Check<'_> {
    pub(super) fn group(&mut self, p: &str, s: &System) {
        self.uses(s.space == Placement::World || !s.points.is_empty() || !s.lock.is_empty(), "particles2");
        self.count(&format!("{p}/points"), s.points.len(), MAX_CONTROL);
        for (k, point) in s.points.iter().enumerate() {
            let q = format!("{p}/points/{k}");
            point.at.iter().for_each(|v| self.value(&format!("{q}/at"), v));
            point.angle.iter().for_each(|v| self.number(&format!("{q}/angle"), v, -360.0, 360.0));
            if point.at.is_some() && point.follow.is_some() {
                self.issues.push(Issue::Source(q));
            }
        }
        if let Some(d) = s.perspective {
            self.uses(true, "particle_depth");
            self.range(&format!("{p}/perspective/focal"), d.focal, 1.0, 1e5);
        }
        if s.lock.iter().enumerate().any(|(i, f)| s.lock[..i].contains(f)) {
            self.issues.push(Issue::Type(format!("{p}/lock")));
        }
        self.count(&format!("{p}/children"), s.children.len(), MAX_CHILDREN);
        for (k, c) in s.children.iter().enumerate() {
            let q = format!("{p}/children/{k}");
            self.uses(true, "particle_children");
            let known = self.scene.particles.contains_key(&c.system);
            self.known(&format!("{q}/system"), "particle system", &c.system, known);
            self.range(&format!("{q}/probability"), c.probability, 0.0, 1.0);
            c.max.iter().for_each(|m| self.range(&format!("{q}/max"), *m as f32, 1.0, MAX_PARTICLES as f32));
            c.offset.iter().for_each(|o| self.place(&format!("{q}/offset"), *o));
            c.angle.iter().for_each(|a| self.range(&format!("{q}/angle"), *a, -360.0, 360.0));
            self.range(&format!("{q}/scale"), c.scale, 0.0, 100.0);
            let repeated = c.inherit.iter().enumerate().any(|(i, f)| c.inherit[..i].contains(f));
            if repeated || (c.follow_inherit && c.on != Moment::Follow) {
                self.issues.push(Issue::Type(format!("{q}/inherit")));
            }
        }
    }

    pub(crate) fn lineage(&mut self) {
        let scene: &Scene = self.scene;
        for id in scene.particles.keys() {
            let mut level = vec![id.as_str()];
            for depth in 0.. {
                let next: Vec<&str> = level
                    .iter()
                    .filter_map(|s| scene.particles.get(*s))
                    .flat_map(|s| s.children.iter().map(|c| c.system.as_str()))
                    .collect();
                if next.is_empty() {
                    break;
                }
                if next.contains(&id.as_str()) {
                    self.issues.push(Issue::Circle(format!("/particles/{id}/children")));
                    break;
                }
                if depth >= 2 {
                    self.issues.push(Issue::Generations(format!("/particles/{id}/children")));
                    break;
                }
                level = next;
            }
        }
    }

    pub(super) fn place(&mut self, path: &str, [x, y]: [f32; 2]) {
        self.range(path, x, -1e6, 1e6);
        self.range(path, y, -1e6, 1e6);
    }
}
