use super::{Collider, Force, Operator, Surface, System};
use crate::validate::{Check, Issue};

impl Check<'_> {
    pub(super) fn operator(&mut self, q: &str, o: &Operator, s: &System) {
        if let Some(w) = o.window {
            self.uses(true, "particles2");
            w.iter().enumerate().for_each(|(k, v)| self.range(&format!("{q}/window/{k}"), *v, 0.0, 1.0));
            if w.windows(2).any(|p| p[0] > p[1]) {
                self.issues.push(Issue::Type(format!("{q}/window")));
            }
        }
        self.force2(q, &o.force, s);
    }

    pub(super) fn collider2(&mut self, q: &str, c: &Collider) {
        self.uses(c.stick, "particle_collisions");
        match &c.bounds {
            Surface::Circle { inside, .. } => self.uses(*inside, "particle_collisions"),
            Surface::Box { from, to, .. } | Surface::Segment { from, to } => {
                self.uses(true, "particle_collisions");
                [*from, *to].into_iter().for_each(|v| self.place(q, v));
                if from == to {
                    self.issues.push(Issue::Type(format!("{q}/bounds")));
                }
            }
            Surface::Layer { id } => {
                self.uses(true, "particle_layers");
                let drawn = self.scene.layer(id).is_some_and(crate::Layer::drawn);
                self.known(&format!("{q}/bounds/layer/id"), "layer", id, drawn);
            }
            _ => {}
        }
    }

    fn force2(&mut self, q: &str, force: &Force, s: &System) {
        let reach = |c: &mut Self, radius: &Option<f32>, falloff: bool| {
            c.uses(radius.is_some() || falloff, "particles2");
            radius.iter().for_each(|r| c.range(&format!("{q}/radius"), *r, 0.0, 1e5));
        };
        match force {
            Force::Gravity(v) => self.value(q, v),
            Force::Drag(v) | Force::Noise(v) => self.value(q, v),
            Force::Turbulence { strength, scale, speed, time_scale, mask } => {
                self.value(&format!("{q}/strength"), strength);
                self.range(&format!("{q}/scale"), *scale, 1e-6, 1.0);
                self.uses(speed.is_some() || time_scale.is_some() || mask.is_some(), "particles2");
                speed.iter().for_each(|v| self.range(&format!("{q}/speed"), *v, 0.0, 100.0));
                time_scale.iter().for_each(|v| self.range(&format!("{q}/time_scale"), *v, 0.0, 100.0));
                mask.iter()
                    .flatten()
                    .enumerate()
                    .for_each(|(k, v)| self.range(&format!("{q}/mask/{k}"), *v, 0.0, 1.0));
            }
            Force::Attractor { target: t, strength, radius, falloff }
            | Force::Orbit { center: t, strength, radius, falloff } => {
                self.value(&format!("{q}/strength"), strength);
                self.target(q, t, s);
                reach(self, radius, falloff.is_some());
            }
            Force::Vortex { center, strength, radius, falloff, inner, outer, ring } => {
                self.value(&format!("{q}/strength"), strength);
                self.target(q, center, s);
                reach(self, radius, falloff.is_some());
                self.uses(inner.is_some() || outer.is_some() || ring.is_some(), "particles2");
                let lo = inner.unwrap_or(0.0);
                self.range(&format!("{q}/inner"), lo, 0.0, 1e5);
                outer.iter().for_each(|o| self.range(&format!("{q}/outer"), *o, lo, 1e5));
                if let Some(r) = ring {
                    self.range(&format!("{q}/ring/radius"), r.radius, 0.0, 1e5);
                    self.range(&format!("{q}/ring/width"), r.width, 0.0, 1e5);
                    self.range(&format!("{q}/ring/pull"), r.pull, -1e3, 1e3);
                }
            }
            Force::Oscillate(o) => {
                self.uses(true, "particles2");
                let p = format!("{q}/oscillate");
                self.range(&format!("{p}/freq/0"), o.freq[0], 0.0, 60.0);
                self.range(&format!("{p}/freq/1"), o.freq[1], o.freq[0], 60.0);
                self.range(&format!("{p}/scale/1"), o.scale[1], o.scale[0], 1e5);
                o.phase.iter().for_each(|v| self.range(&format!("{p}/phase"), *v, 0.0, 1.0));
                o.mask.iter().flatten().for_each(|v| self.range(&format!("{p}/mask"), *v, 0.0, 1.0));
            }
            Force::Cap(v) | Force::Torque(v) | Force::SpinDrag(v) => {
                self.uses(true, "particles2");
                self.value(q, v);
            }
            Force::SlowNear(n) => {
                self.uses(true, "particles2");
                self.target(q, &n.target, s);
                self.range(&format!("{q}/slow_near/inner"), n.inner, 0.0, 1e5);
                self.range(&format!("{q}/slow_near/outer"), n.outer, n.inner, 1e5);
                self.range(&format!("{q}/slow_near/factor"), n.factor, 0.0, 1.0);
            }
            Force::KeepDistance(k) => {
                self.uses(true, "particle_fields");
                self.target(q, &k.target, s);
                self.range(&format!("{q}/keep_distance/distance"), k.distance, 0.0, 1e5);
                self.range(&format!("{q}/keep_distance/strength"), k.strength, -1e3, 1e3);
            }
            Force::KeepBetween(k) => {
                self.uses(true, "particle_fields");
                self.target(q, &k.from, s);
                self.target(q, &k.to, s);
                self.range(&format!("{q}/keep_between/strength"), k.strength, -1e3, 1e3);
            }
            Force::Boids(b) => {
                self.uses(true, "boids");
                let p = format!("{q}/boids");
                self.range(&format!("{p}/radius"), b.radius, 1.0, 1e4);
                for (name, v) in
                    [("separation", b.separation), ("alignment", b.alignment), ("cohesion", b.cohesion)]
                {
                    self.range(&format!("{p}/{name}"), v, 0.0, 10.0);
                }
                self.range(&format!("{p}/max_speed"), b.max_speed, 0.0, 1e5);
                let system = q.split("/forces/").next().unwrap_or(q);
                self.range(&format!("{system}/capacity"), s.capacity as f32, 1.0, 8192.0);
            }
            Force::Remap(r) => self.remap(&format!("{q}/remap"), r, s),
        }
    }
}
