use crate::validate::{Check, Issue};
use crate::value::{Band, Param, Value, Vec2};
use crate::{Color, Ease, Kind, Layer, MAX_DEPTH, MAX_EFFECTS, Mask, Shape, descriptor};
use std::collections::HashMap;

pub(crate) trait Probe: Sized {
    const TEXT: bool = false;
    fn probe(_: &mut Check, _: &str, _: &Self) {}
}

impl Probe for f32 {
    fn probe(c: &mut Check, path: &str, v: &f32) {
        c.range(path, *v, -1e6, 1e6);
    }
}

impl Probe for bool {}

impl Probe for String {
    const TEXT: bool = true;
}

impl Probe for Vec2 {
    fn probe(c: &mut Check, path: &str, v: &Vec2) {
        v.0.iter().enumerate().for_each(|(i, x)| c.range(&format!("{path}/{i}"), *x, -1e6, 1e6));
    }
}

impl Probe for Color {
    fn probe(c: &mut Check, path: &str, v: &Color) {
        c.color(path, *v);
    }
}

impl Probe for Param {
    fn probe(c: &mut Check, path: &str, v: &Param) {
        match v {
            Param::Number(n) => f32::probe(c, path, n),
            Param::Vec2(v) => Vec2::probe(c, path, &Vec2(*v)),
            Param::Color(color) => c.color(path, *color),
            Param::Bool(_) | Param::Text(_) => {}
        }
    }
}

impl Check<'_> {
    pub(crate) fn value<T: Probe>(&mut self, path: &str, value: &Value<T>) {
        let b = match value {
            Value::Fixed(v) => return T::probe(self, path, v),
            Value::Bound(b) => b,
        };
        if b.sources() != 1 {
            self.issues.push(Issue::Source(path.into()));
        }
        if let Some(id) = &b.prop {
            self.known(path, "property", id, self.props.contains_key(id));
        }
        let misplaced = (b.map.is_some() && b.prop.is_none()) || (b.clock.is_some() && !T::TEXT);
        if misplaced || (T::TEXT && (b.audio.is_some() || b.cursor.is_some())) {
            self.issues.push(Issue::Type(path.into()));
        }
        if let Some(keys) = &b.keys {
            self.keys_sorted(&format!("{path}/keys"), keys.iter().map(|k| k.t));
            for (i, k) in keys.iter().enumerate() {
                T::probe(self, &format!("{path}/keys/{i}/1"), &k.v);
                let Ease::Bezier(curve) = k.ease else { continue };
                for (j, v) in curve.into_iter().enumerate() {
                    let [lo, hi] = if j % 2 == 0 { [0.0, 1.0] } else { [-1e3, 1e3] };
                    self.range(&format!("{path}/keys/{i}/2/{j}"), v, lo, hi);
                }
            }
        }
        if let Some(id) = &b.timeline {
            self.known(
                &format!("{path}/timeline"),
                "timeline",
                id,
                self.scene.timelines.iter().any(|t| &t.id == id),
            );
        }
        if let Some(Band::Index(i)) = b.audio {
            self.range(&format!("{path}/audio"), i as f32, 0.0, 63.0);
        }
        for (name, v) in [("scale", b.scale), ("offset", b.offset)] {
            v.into_iter().for_each(|v| self.range(&format!("{path}/{name}"), v, -1e6, 1e6));
        }
        if let Some([lo, hi]) = b.clamp {
            self.range(&format!("{path}/clamp/0"), lo, -1e6, 1e6);
            self.range(&format!("{path}/clamp/1"), hi, lo, 1e6);
        }
        if let Some(tau) = b.smooth {
            self.range(&format!("{path}/smooth"), tau, 0.0, 60.0);
        }
    }

    fn asset(&mut self, path: &str, id: &str) {
        self.known(path, "asset", id, self.scene.assets.contains_key(id));
    }

    pub(crate) fn layers(&mut self, base: &str, layers: &[Layer], depth: usize) {
        for (i, layer) in layers.iter().enumerate() {
            let p = format!("{base}/{i}");
            if depth > MAX_DEPTH {
                self.issues.push(Issue::Depth(p.clone()));
                continue;
            }
            self.value(&format!("{p}/opacity"), &layer.opacity);
            if let Value::Fixed(o) = layer.opacity {
                self.range(&format!("{p}/opacity"), o, 0.0, 1.0);
            }
            self.value(&format!("{p}/visible"), &layer.visible);
            let t = &layer.transform;
            t.position.iter().for_each(|v| self.value(&format!("{p}/transform/position"), v));
            self.value(&format!("{p}/transform/scale"), &t.scale);
            self.value(&format!("{p}/transform/rotation"), &t.rotation);
            self.value(&format!("{p}/transform/skew"), &t.skew);
            t.origin.iter().for_each(|o| Vec2::probe(self, &format!("{p}/transform/origin"), &Vec2(*o)));
            self.range(&format!("{p}/parallax"), layer.parallax, -10.0, 10.0);
            if let Some(mask) = &layer.mask {
                self.mask(&format!("{p}/mask"), mask);
            }
            self.effects(&p, layer);
            self.kind(&p, &layer.kind, depth);
        }
    }

    fn effects(&mut self, p: &str, layer: &Layer) {
        self.count(&format!("{p}/effects"), layer.effects.len(), MAX_EFFECTS);
        for (i, effect) in layer.effects.iter().enumerate() {
            let e = format!("{p}/effects/{i}");
            self.value(&format!("{e}/enabled"), &effect.enabled);
            if let Some(mask) = &effect.mask {
                self.mask(&format!("{e}/mask"), mask);
                if mask.layer.is_some() || mask.shape.is_some() {
                    self.issues.push(Issue::EffectMask(format!("{e}/mask")));
                }
            }
            let Some(d) = descriptor(&effect.effect) else {
                self.known(&format!("{e}/effect"), "effect", &effect.effect, false);
                continue;
            };
            for (id, value) in &effect.params {
                let path = format!("{e}/params/{id}");
                let Some(def) = d.params.iter().find(|p| &p.id == id) else {
                    self.known(&path, "parameter", id, false);
                    continue;
                };
                self.value(&path, value);
                match value {
                    Value::Fixed(v) if !def.accepts(v) => self.issues.push(Issue::Type(path)),
                    Value::Fixed(Param::Number(n)) => {
                        self.range(&path, *n, def.min.unwrap_or(f32::MIN), def.max.unwrap_or(f32::MAX))
                    }
                    _ => {}
                }
            }
        }
    }

    fn kind(&mut self, p: &str, kind: &Kind, depth: usize) {
        match kind {
            Kind::Group { layers, .. } => self.layers(&format!("{p}/layers"), layers, depth + 1),
            Kind::Image { image, size } => {
                self.value(&format!("{p}/image"), image);
                match image {
                    Value::Fixed(id) => self.asset(&format!("{p}/image"), id),
                    Value::Bound(b) => b
                        .map
                        .iter()
                        .flatten()
                        .for_each(|(k, id)| self.asset(&format!("{p}/image/map/{k}"), id)),
                }
                self.size(&format!("{p}/size"), *size);
            }
            Kind::Video { video, size } => {
                self.asset(&format!("{p}/video"), video);
                self.size(&format!("{p}/size"), *size);
            }
            Kind::Text { text, size, color, .. } => {
                self.value(&format!("{p}/text"), text);
                self.value(&format!("{p}/color"), color);
                self.range(&format!("{p}/size"), *size, 1.0, 1024.0);
            }
            Kind::Shape { shape, fill, gradient, stroke } => {
                self.shape(&format!("{p}/shape"), shape);
                fill.iter().for_each(|f| self.value(&format!("{p}/fill"), f));
                if let Some(g) = gradient {
                    self.range(&format!("{p}/gradient/stops"), g.stops.len() as f32, 2.0, 4.0);
                    self.keys_sorted(&format!("{p}/gradient/stops"), g.stops.iter().map(|s| s.0));
                    Vec2::probe(self, &format!("{p}/gradient/from"), &Vec2(g.from));
                    Vec2::probe(self, &format!("{p}/gradient/to"), &Vec2(g.to));
                    for (i, (_, color)) in g.stops.iter().enumerate() {
                        self.color(&format!("{p}/gradient/stops/{i}/1"), *color);
                    }
                }
                if let Some(s) = stroke {
                    self.value(&format!("{p}/stroke/color"), &s.color);
                    self.range(&format!("{p}/stroke/width"), s.width, 0.0, 1e4);
                }
            }
            Kind::Particles { system } => self.known(
                &format!("{p}/system"),
                "particle system",
                system,
                self.scene.particles.contains_key(system),
            ),
            Kind::Spectrum(s) => {
                if ![16, 32, 64].contains(&s.bands) {
                    self.issues.push(Issue::Range {
                        path: format!("{p}/bands"),
                        value: s.bands as f32,
                        min: 16.0,
                        max: 64.0,
                    });
                }
                self.size(&format!("{p}/size"), Some(s.size));
                self.range(&format!("{p}/gap"), s.gap, 0.0, 0.95);
                self.value(&format!("{p}/color"), &s.color);
            }
            Kind::Solid { color, size } => {
                self.value(&format!("{p}/color"), color);
                self.size(&format!("{p}/size"), *size);
            }
            Kind::Adjustment {} => {}
        }
    }

    fn size(&mut self, path: &str, size: Option<[f32; 2]>) {
        size.iter()
            .flatten()
            .enumerate()
            .for_each(|(i, v)| self.range(&format!("{path}/{i}"), *v, 1e-3, 1e5));
    }

    fn shape(&mut self, path: &str, shape: &Shape) {
        match *shape {
            Shape::Rect { size, radius } => {
                self.size(&format!("{path}/rect/size"), Some(size));
                self.range(&format!("{path}/rect/radius"), radius, 0.0, size[0].min(size[1]) / 2.0);
            }
            Shape::Ellipse { size } => self.size(&format!("{path}/ellipse/size"), Some(size)),
            Shape::Ring { size, width } => {
                self.size(&format!("{path}/ring/size"), Some(size));
                self.range(&format!("{path}/ring/width"), width, 1e-3, size[0].min(size[1]) / 2.0);
            }
            Shape::Polygon { sides, size } => {
                self.range(&format!("{path}/polygon/sides"), sides as f32, 3.0, 64.0);
                self.range(&format!("{path}/polygon/size"), size, 1e-3, 1e5);
            }
            Shape::Star { points, size, inner } => {
                self.range(&format!("{path}/star/points"), points as f32, 2.0, 64.0);
                self.range(&format!("{path}/star/size"), size, 1e-3, 1e5);
                self.range(&format!("{path}/star/inner"), inner, 0.01, 1.0);
            }
            Shape::Line { to, width } => {
                to.iter()
                    .enumerate()
                    .for_each(|(i, v)| self.range(&format!("{path}/line/to/{i}"), *v, -1e5, 1e5));
                self.range(&format!("{path}/line/width"), width, 1e-3, 1e4);
            }
        }
    }

    fn mask(&mut self, path: &str, mask: &Mask) {
        let sources = [mask.texture.is_some(), mask.layer.is_some(), mask.shape.is_some()];
        if sources.iter().filter(|s| **s).count() != 1 {
            self.issues.push(Issue::Source(path.into()));
        }
        if let Some(id) = &mask.texture {
            self.asset(&format!("{path}/texture"), id);
        }
        if let Some(id) = &mask.layer {
            self.known(&format!("{path}/layer"), "layer", id, self.scene.layer(id).is_some());
        }
        if let Some(shape) = &mask.shape {
            self.shape(&format!("{path}/shape"), shape);
        }
        self.range(&format!("{path}/feather"), mask.feather, 0.0, 1e4);
    }

    pub(crate) fn masks(&mut self) {
        let scene = self.scene;
        let mut targets = HashMap::new();
        for layer in scene.layers() {
            let target = layer.mask.as_ref().and_then(|m| m.layer.as_deref());
            targets.entry(layer.id.as_str()).or_insert(target);
        }
        for layer in scene.layers() {
            let mut current = targets.get(layer.id.as_str()).copied().flatten();
            for _ in 0..crate::MAX_LAYERS {
                match current {
                    Some(id) if id == layer.id => {
                        self.issues.push(Issue::Cycle(format!("/layers/{}", layer.id)));
                        break;
                    }
                    Some(id) => current = targets.get(id).copied().flatten(),
                    None => break,
                }
            }
        }
    }
}
