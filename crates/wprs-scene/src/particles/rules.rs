use super::{
    EmitShape, MAX_COLLIDERS, MAX_COLLIDERS2, MAX_EMITTERS, MAX_EMITTERS2, MAX_FORCES, MAX_FORCES2,
    MAX_PREWARM, MAX_PREWARM2, MAX_RATE, System,
};
use crate::MAX_PARTICLES;
use crate::validate::{Check, Issue};
use crate::value::Value;

impl Check<'_> {
    pub(crate) fn system(&mut self, p: &str, s: &System) {
        self.range(&format!("{p}/capacity"), s.capacity as f32, 1.0, MAX_PARTICLES as f32);
        self.range(&format!("{p}/prewarm"), s.prewarm, 0.0, MAX_PREWARM2);
        if s.emitters.is_empty() {
            self.issues.push(Issue::Empty(format!("{p}/emitters")));
        }
        self.count(&format!("{p}/emitters"), s.emitters.len(), MAX_EMITTERS2);
        self.count(&format!("{p}/forces"), s.forces.len(), MAX_FORCES2);
        self.count(&format!("{p}/collide"), s.collide.len(), MAX_COLLIDERS2);
        let wide = [
            s.prewarm > MAX_PREWARM,
            s.emitters.len() > MAX_EMITTERS,
            s.forces.len() > MAX_FORCES,
            s.collide.len() > MAX_COLLIDERS,
        ];
        self.uses(wide.contains(&true), "particles2");
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
                EmitShape::Line(ab) => ab.to_vec(),
                EmitShape::Circle { center, radius, inner, .. } => {
                    self.num(&format!("{q}/shape/circle/radius"), radius, 0.0, 1e5);
                    self.num(
                        &format!("{q}/shape/circle/inner"),
                        inner,
                        0.0,
                        radius.1.as_ref().map_or(radius.0, |_| 1e5),
                    );
                    vec![*center]
                }
                EmitShape::Mask { texture, threshold } => {
                    let known = self.scene.assets.contains_key(texture);
                    self.known(&format!("{q}/shape/mask/texture"), "asset", texture, known);
                    self.num(&format!("{q}/shape/mask/threshold"), threshold, 0.0, 1.0);
                    if threshold.1.is_some() {
                        self.issues.push(Issue::Type(format!("{q}/shape/mask/threshold")));
                    }
                    vec![]
                }
                _ => vec![],
            };
            points.iter().for_each(|v| self.place(&format!("{q}/shape"), *v));
            self.emission(&q, e, s);
        }
        let i = &s.init;
        let spans = [
            ("life", &i.life, 1e-3, 600.0),
            ("speed", &i.speed, 0.0, 1e5),
            ("angle", &i.angle, -3600.0, 3600.0),
            ("size", &i.size, 0.0, 4096.0),
            ("rotation", &i.rotation, -3600.0, 3600.0),
            ("spin", &i.spin, -1e5, 1e5),
            ("frame", &i.frame, 0.0, 4096.0),
        ];
        for (name, spread, min, max) in spans {
            self.spread(&format!("{p}/init/{name}"), spread, min, max);
        }
        let fixed = [
            ("color", i.color.bound.is_some()),
            ("velocity", i.velocity.as_ref().is_some_and(|v| v.bound.is_some())),
        ];
        fixed
            .iter()
            .filter(|(_, b)| *b)
            .for_each(|(name, _)| self.issues.push(Issue::Type(format!("{p}/init/{name}"))));
        i.color.ends.iter().enumerate().for_each(|(k, c)| self.color(&format!("{p}/init/color/{k}"), *c));
        self.init2(&format!("{p}/init"), i, s);
        for (k, o) in s.forces.iter().enumerate() {
            self.operator(&format!("{p}/forces/{k}"), o, s);
        }
        for (k, c) in s.collide.iter().enumerate() {
            let q = format!("{p}/collide/{k}");
            self.range(&format!("{q}/bounce"), c.bounce, 0.0, 1.0);
            self.range(&format!("{q}/friction"), c.friction, 0.0, 1.0);
            match &c.bounds {
                super::Surface::Plane { point, normal } => {
                    self.place(&q, *point);
                    self.range(&format!("{q}/bounds/plane/normal"), normal[0].hypot(normal[1]), 1e-6, 1e6);
                }
                super::Surface::Circle { center, radius, .. } => {
                    self.place(&q, *center);
                    self.range(&format!("{q}/bounds/circle/radius"), *radius, 1e-3, 1e5);
                }
                _ => {}
            }
            self.collider2(&q, c);
        }
        self.render(p, &s.render);
        self.group(p, s);
    }
}
