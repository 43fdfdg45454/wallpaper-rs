use super::{EmitShape, Emitter, Handle, Instance, Num, Spread, System, Target};
use crate::Value;
use crate::validate::{Check, Issue};

impl Check<'_> {
    pub(crate) fn instance(&mut self, p: &str, i: &Instance, s: Option<&System>) {
        self.uses(true, "particles2");
        let numbers = [
            ("alpha", &i.alpha, 1.0),
            ("size", &i.size, 100.0),
            ("count", &i.count, 1.0),
            ("speed", &i.speed, 100.0),
            ("life", &i.life, 100.0),
            ("time_scale", &i.time_scale, 100.0),
            ("brightness", &i.brightness, 16.0),
        ];
        for (name, v, max) in numbers {
            v.iter().for_each(|v| self.number(&format!("{p}/{name}"), v, 0.0, max));
        }
        i.color.iter().for_each(|c| self.value(&format!("{p}/color"), c));
        self.count(&format!("{p}/points"), i.points.len(), super::MAX_CONTROL);
        i.points.iter().enumerate().for_each(|(k, v)| self.value(&format!("{p}/points/{k}"), v));
        let locked = s.map(|s| s.lock.as_slice()).unwrap_or_default();
        for field in i.fields().into_iter().filter(|f| locked.contains(f)) {
            let name = serde_json::to_value(field)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            self.issues.push(Issue::Locked(format!("{p}/{name}")));
        }
    }

    pub(super) fn handle(&mut self, path: &str, h: Handle, s: &System) {
        self.uses(true, "particles2");
        if h.0 != 0 && usize::from(h.0) >= s.points.len() {
            self.known(path, "control point", &String::from(h), false);
        }
    }

    pub(super) fn target(&mut self, path: &str, t: &Target, s: &System) {
        match t {
            Target::Point(at) => self.place(path, *at),
            Target::Control(h) => self.handle(path, *h, s),
            Target::Follow(_) => {}
        }
    }

    pub(super) fn spread(&mut self, path: &str, v: &Spread<f32>, min: f32, max: f32) {
        let [lo, hi] = v.ends;
        match v.bound.as_deref() {
            Some(ends) => {
                self.uses(true, "particle_values");
                ends.iter().enumerate().for_each(|(i, end)| self.value(&format!("{path}/{i}"), end));
            }
            None => {
                self.range(&format!("{path}/0"), lo, min, max);
                self.range(&format!("{path}/1"), hi, lo.max(min), max);
            }
        }
        self.exponent(path, v.exponent);
    }

    pub(super) fn num(&mut self, path: &str, n: &Num, min: f32, max: f32) {
        match &n.1 {
            Some(b) => {
                self.uses(true, "particle_values");
                self.value(path, &Value::Bound(b.clone()));
            }
            None => self.range(path, n.0, min, max),
        }
    }

    pub(super) fn exponent(&mut self, path: &str, e: Option<f32>) {
        if let Some(e) = e {
            self.uses(true, "particles2");
            self.range(&format!("{path}/2"), e, 0.1, 10.0);
        }
    }

    pub(super) fn emission(&mut self, q: &str, e: &Emitter, s: &System) {
        let timed = [e.duration.is_some(), e.delay.is_some(), e.periodic.is_some(), e.max_per_step.is_some()];
        self.uses(timed.contains(&true) || e.audio_gate, "particles2");
        e.duration.iter().for_each(|d| self.range(&format!("{q}/duration"), *d, 0.0, 86_400.0));
        e.delay.iter().for_each(|d| self.range(&format!("{q}/delay"), *d, 0.0, 86_400.0));
        e.max_per_step.iter().for_each(|n| self.range(&format!("{q}/max_per_step"), *n as f32, 1.0, 1000.0));
        if let Some(w) = e.periodic {
            for (name, [lo, hi]) in [("on", w.on), ("off", w.off)] {
                self.range(&format!("{q}/periodic/{name}/0"), lo, 1.0 / 60.0, 3600.0);
                self.range(&format!("{q}/periodic/{name}/1"), hi, lo, 3600.0);
            }
        }
        let p = format!("{q}/shape");
        match &e.shape {
            EmitShape::Box(a) => {
                [a.from, a.to].into_iter().for_each(|v| self.place(&p, v));
                let size = [(a.to[0] - a.from[0]).abs(), (a.to[1] - a.from[1]).abs()];
                for (i, v) in a.inner.iter().flatten().enumerate() {
                    self.range(&format!("{p}/box/inner/{i}"), *v, 0.0, size[i]);
                }
                self.uses(a.inner.is_some(), "particles2");
                a.at.iter().for_each(|h| self.handle(&format!("{p}/box/at"), *h, s));
            }
            EmitShape::Circle { scale, sign, at, .. } => {
                self.uses(scale.is_some() || sign.is_some(), "particles2");
                for (i, v) in scale.iter().flatten().enumerate() {
                    self.range(&format!("{p}/circle/scale/{i}"), *v, 0.01, 100.0);
                }
                for (i, v) in sign.iter().flatten().enumerate() {
                    self.range(&format!("{p}/circle/sign/{i}"), f32::from(*v), -1.0, 1.0);
                }
                at.iter().for_each(|h| self.handle(&format!("{p}/circle/at"), *h, s));
            }
            EmitShape::Layer(l) => {
                self.uses(true, "particle_layers");
                let drawn = self.scene.layer(&l.id).is_some_and(crate::Layer::drawn);
                self.known(&format!("{p}/layer/id"), "layer", &l.id, drawn);
                self.num(&format!("{p}/layer/threshold"), &l.threshold, 0.0, 1.0);
                self.range(&format!("{p}/layer/jitter"), l.jitter, 0.0, 1e4);
                let id = q.split('/').nth(2).unwrap_or_default();
                if self.scene.layer(&l.id).is_some_and(|src| holds(src, id)) {
                    self.issues.push(Issue::Circle(format!("{p}/layer/id")));
                }
            }
            EmitShape::Around(a) => {
                self.uses(true, "particle_fields");
                a.at.iter().for_each(|t| self.target(&format!("{p}/around/at"), t, s));
                self.range(&format!("{p}/around/count"), a.count as f32, 1.0, 4096.0);
                self.num(&format!("{p}/around/arc"), &a.arc, 0.0, 360.0);
                self.num(&format!("{p}/around/radius"), &a.radius, 0.0, 1e5);
                self.num(&format!("{p}/around/speed"), &a.speed, -1e4, 1e4);
            }
            EmitShape::Between(b) => {
                self.uses(true, "particle_fields");
                self.target(&format!("{p}/between/from"), &b.from, s);
                self.target(&format!("{p}/between/to"), &b.to, s);
                self.range(&format!("{p}/between/count"), b.count as f32, 1.0, 4096.0);
                self.num(&format!("{p}/between/span"), &b.span, 0.0, 1.0);
                if let Some(arc) = b.arc {
                    self.range(&format!("{p}/between/arc/amount"), arc.amount, -10.0, 10.0);
                    self.range(&format!("{p}/between/arc/side"), arc.side, -1.0, 1.0);
                }
                if let Some(t) = b.taper {
                    self.range(&format!("{p}/between/taper/position"), t.position, 0.0, 1.0);
                    self.range(&format!("{p}/between/taper/speed"), t.speed, -100.0, 100.0);
                    self.range(&format!("{p}/between/taper/size"), t.size, 0.0, 1.0);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn holds(layer: &crate::Layer, system: &str) -> bool {
    let draws = matches!(&layer.kind, crate::Kind::Particles { system: s, .. } if s == system);
    draws || layer.children().iter().any(|c| holds(c, system))
}
