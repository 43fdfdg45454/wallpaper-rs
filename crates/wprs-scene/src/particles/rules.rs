use super::{EmitShape, MAX_COLLIDERS, MAX_EMITTERS, MAX_FORCES, MAX_PREWARM, MAX_RATE, System};
use crate::MAX_PARTICLES;
use crate::validate::{Check, Issue};
use crate::value::Value;

impl Check<'_> {
    pub(crate) fn system(&mut self, p: &str, s: &System) {
        self.range(&format!("{p}/capacity"), s.capacity as f32, 1.0, MAX_PARTICLES as f32);
        self.range(&format!("{p}/prewarm"), s.prewarm, 0.0, MAX_PREWARM);
        if s.emitters.is_empty() {
            self.issues.push(Issue::Empty(format!("{p}/emitters")));
        }
        self.count(&format!("{p}/emitters"), s.emitters.len(), MAX_EMITTERS);
        self.count(&format!("{p}/forces"), s.forces.len(), MAX_FORCES);
        self.count(&format!("{p}/collide"), s.collide.len(), MAX_COLLIDERS);
        for (i, e) in s.emitters.iter().enumerate() {
            let q = format!("{p}/emitters/{i}");
            self.value(&format!("{q}/rate"), &e.rate);
            if let Value::Fixed(rate) = e.rate {
                self.range(&format!("{q}/rate"), rate, 0.0, MAX_RATE);
            }
            if let Some(b) = &e.burst {
                self.range(&format!("{q}/burst/count"), b.count as f32, 1.0, MAX_PARTICLES as f32);
                b.every
                    .iter()
                    .for_each(|t| self.range(&format!("{q}/burst/every"), *t, 1.0 / 60.0, 86_400.0));
            }
            self.range(&format!("{q}/inherit"), e.inherit, 0.0, 4.0);
            let points = match &e.shape {
                EmitShape::Point(a) => vec![*a],
                EmitShape::Box(ab) | EmitShape::Line(ab) => ab.to_vec(),
                EmitShape::Circle { center, radius, inner } => {
                    self.range(&format!("{q}/shape/circle/radius"), *radius, 0.0, 1e5);
                    self.range(&format!("{q}/shape/circle/inner"), *inner, 0.0, *radius);
                    vec![*center]
                }
                EmitShape::Edge => vec![],
                EmitShape::Mask { texture, threshold } => {
                    let known = self.scene.assets.contains_key(texture);
                    self.known(&format!("{q}/shape/mask/texture"), "asset", texture, known);
                    self.range(&format!("{q}/shape/mask/threshold"), *threshold, 0.0, 1.0);
                    vec![]
                }
            };
            points.iter().for_each(|v| self.place(&format!("{q}/shape"), *v));
        }
        let i = &s.init;
        let spans = [
            ("life", i.life, 1e-3, 600.0),
            ("speed", i.speed, 0.0, 1e5),
            ("angle", i.angle, -3600.0, 3600.0),
            ("size", i.size, 0.0, 4096.0),
            ("rotation", i.rotation, -3600.0, 3600.0),
            ("spin", i.spin, -1e5, 1e5),
            ("frame", i.frame, 0.0, 4096.0),
        ];
        for (name, [lo, hi], min, max) in spans {
            self.range(&format!("{p}/init/{name}/0"), lo, min, max);
            self.range(&format!("{p}/init/{name}/1"), hi, lo.max(min), max);
        }
        i.color.iter().enumerate().for_each(|(k, c)| self.color(&format!("{p}/init/color/{k}"), *c));
        self.forces(p, s);
        self.render(p, &s.render);
    }

    pub(super) fn place(&mut self, path: &str, [x, y]: [f32; 2]) {
        self.range(path, x, -1e6, 1e6);
        self.range(path, y, -1e6, 1e6);
    }
}
