use crate::property::fits;
use crate::validate::{Check, FAR};
use crate::{
    Emitter, Issue, Layer, LayerKind, MAX_EFFECTS, MAX_KEYFRAMES, MAX_PARTICLES, MAX_RATE, MAX_STOPS,
    MAX_TIME, Property, PropertyKind, Timeline, Value,
};
use serde_json::Value as Json;

impl Check {
    pub(crate) fn layer(&mut self, p: &str, l: &Layer) {
        let t = l.transform;
        self.params(
            p,
            &[
                ("opacity", l.opacity, 0.0, 1.0),
                ("transform/x", t.x, -FAR, FAR),
                ("transform/y", t.y, -FAR, FAR),
                ("transform/scale", t.scale, 0.001, 1000.0),
                ("transform/rotation", t.rotation, -FAR, FAR),
                ("transform/origin/0", t.origin[0], 0.0, 1.0),
                ("transform/origin/1", t.origin[1], 0.0, 1.0),
            ],
        );
        let k = format!("{p}/kind");
        match &l.kind {
            LayerKind::Image { src } | LayerKind::Video { src } => self.filled(format!("{k}/src"), src),
            LayerKind::Color { color } => self.color(&format!("{k}/color"), *color),
            LayerKind::Text { size, color, .. } => {
                self.params(&k, &[("size", *size, 1.0, 1024.0)]);
                self.color(&format!("{k}/color"), *color);
            }
            LayerKind::Particles(e) => self.emitter(&k, e),
        }
        self.count(&format!("{p}/effects"), l.effects.len(), MAX_EFFECTS);
        l.effects.iter().enumerate().for_each(|(i, e)| self.params(&format!("{p}/effects/{i}"), &e.spec().1));
        if let Some(mask) = &l.mask {
            self.filled(format!("{p}/mask"), mask);
        }
    }

    fn emitter(&mut self, k: &str, e: &Emitter) {
        let ([life0, life1], [speed0, speed1], [size0, size1]) = (e.lifetime, e.speed, e.size);
        self.params(
            k,
            &[
                ("rate", e.rate, 0.0, MAX_RATE),
                ("lifetime/0", life0, 0.01, 600.0),
                ("lifetime/1", life1, life0, 600.0),
                ("speed/0", speed0, 0.0, 1e5),
                ("speed/1", speed1, speed0, 1e5),
                ("direction", e.direction, -360.0, 360.0),
                ("spread", e.spread, 0.0, 360.0),
                ("gravity/0", e.gravity[0], -1e5, 1e5),
                ("gravity/1", e.gravity[1], -1e5, 1e5),
                ("size/0", size0, 0.1, 4096.0),
                ("size/1", size1, size0, 4096.0),
                ("max_particles", e.max_particles as f32, 1.0, MAX_PARTICLES as f32),
            ],
        );
        let g = format!("{k}/gradient");
        if e.gradient.is_empty() {
            self.0.push(Issue::Empty(g.clone()));
        }
        self.count(&g, e.gradient.len(), MAX_STOPS);
        e.gradient.iter().enumerate().for_each(|(i, c)| self.color(&format!("{g}/{i}"), *c));
        if let Some(texture) = &e.texture {
            self.filled(format!("{k}/texture"), texture);
        }
    }

    pub(crate) fn property(&mut self, json: &Json, p: &str, prop: &Property) {
        let k = format!("{p}/kind");
        match &prop.kind {
            PropertyKind::Color { value } => self.color(&format!("{k}/value"), *value),
            PropertyKind::Slider { value, min, max, step } => self.params(
                &k,
                &[
                    ("min", *min, -FAR, FAR),
                    ("max", *max, *min, FAR),
                    ("step", *step, 1e-6, FAR),
                    ("value", *value, *min, *max),
                ],
            ),
            PropertyKind::Combo { value, options } if !options.contains(value) => {
                self.0.push(Issue::Choice(format!("{k}/value")))
            }
            _ => {}
        }
        let value = prop.value();
        for (i, target) in prop.targets.iter().enumerate().filter(|(_, t)| !fits(json, t, &value)) {
            self.0.push(Issue::Binding { path: format!("{p}/targets/{i}"), target: target.clone() });
        }
    }

    pub(crate) fn timeline(&mut self, json: &Json, p: &str, t: &Timeline) {
        let k = format!("{p}/keyframes");
        if t.keyframes.is_empty() {
            self.0.push(Issue::Empty(k.clone()));
        }
        self.count(&k, t.keyframes.len(), MAX_KEYFRAMES);
        if t.keyframes.windows(2).any(|w| w[0].0 >= w[1].0) {
            self.0.push(Issue::Unsorted(k.clone()));
        }
        for (i, (time, value)) in t.keyframes.iter().enumerate() {
            self.params(&format!("{k}/{i}"), &[("0", *time, 0.0, MAX_TIME)]);
            match value {
                Value::Number(n) => self.params(&format!("{k}/{i}"), &[("1", *n, -FAR, FAR)]),
                Value::Color(c) => self.color(&format!("{k}/{i}/1"), *c),
                _ => {}
            }
        }
        if t.keyframes.iter().any(|(_, v)| !fits(json, &t.target, v)) {
            self.0.push(Issue::Binding { path: format!("{p}/target"), target: t.target.clone() });
        }
    }
}
