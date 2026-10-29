use crate::probe::Probe;
use crate::validate::{Check, Issue};
use crate::value::{Param, Value, Vec2};
use crate::{Kind, Layer, MAX_DEPTH, MAX_EFFECTS, Mask, ParamKind, Shape};
use std::collections::HashMap;

impl Check<'_> {
    fn texture(&mut self, path: &str, id: &str, lut: bool) {
        self.moving(&Value::Fixed(id.into()));
        let cube = self.scene.assets.get(id).map(|a| a.path.to_lowercase().ends_with(".cube"));
        self.known(path, "asset", id, cube.is_some());
        if cube.is_some_and(|cube| cube != lut) {
            self.issues.push(Issue::Type(path.into()));
        }
    }

    fn asset(&mut self, path: &str, id: &str) {
        self.known(path, "asset", id, self.scene.assets.contains_key(id));
    }

    pub(crate) fn named(&mut self, path: &str, named: &Value<String>) {
        self.value(path, named);
        match named {
            Value::Fixed(id) => self.asset(path, id),
            Value::Bound(b) => {
                b.map.iter().flatten().for_each(|(k, id)| self.asset(&format!("{path}/map/{k}"), id))
            }
        }
    }

    pub(crate) fn layers(&mut self, base: &str, layers: &[Layer], depth: usize) {
        for (i, layer) in layers.iter().enumerate() {
            self.layer(&format!("{base}/{i}"), layer, depth);
        }
    }

    pub(crate) fn layer(&mut self, p: &str, layer: &Layer, depth: usize) {
        if depth > MAX_DEPTH {
            return self.issues.push(Issue::Depth(p.into()));
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
        for (i, v) in layer.parallax.0.into_iter().enumerate() {
            self.range(&format!("{p}/parallax/{i}"), v, -10.0, 10.0);
        }
        if let Some(mask) = &layer.mask {
            self.mask(&format!("{p}/mask"), mask);
        }
        self.fields(p, layer);
        self.hooks(p, layer);
        self.effects(p, layer);
        self.kind(p, &layer.kind, depth);
        match &layer.kind {
            Kind::Sound(sound) => self.sound(p, layer, sound),
            Kind::Light(light) => self.light(p, layer, light),
            Kind::Mesh(mesh) => self.mesh(p, layer, mesh),
            Kind::Model(model) => self.model(p, layer, model),
            _ => {}
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
            let scene = self.scene;
            let Some(d) = scene.effect(&effect.effect) else {
                let declared = scene.effects.contains_key(&effect.effect);
                self.known(&format!("{e}/effect"), "effect", &effect.effect, declared);
                continue;
            };
            if !d.stage.is_layer() {
                self.issues.push(Issue::Type(format!("{e}/effect")));
            }
            self.variants(&e, d, effect);
            for (id, value) in &effect.params {
                let path = format!("{e}/params/{id}");
                if d.options.iter().any(|o| &o.id == id) {
                    continue;
                }
                let Some(def) = d.params.iter().find(|p| &p.id == id) else {
                    self.known(&path, "parameter", id, false);
                    continue;
                };
                self.value(&path, value);
                let lut = def.kind == ParamKind::Lut;
                match value {
                    Value::Fixed(v) if !def.accepts(v) => self.issues.push(Issue::Type(path)),
                    Value::Fixed(Param::Number(n)) => {
                        self.range(&path, *n, def.min.unwrap_or(f32::MIN), def.max.unwrap_or(f32::MAX))
                    }
                    Value::Fixed(Param::Text(id)) if def.kind.asset() => self.texture(&path, id, lut),
                    Value::Bound(b) if b.layer.is_some() => self.fed(&path, def, b),
                    Value::Bound(b) if def.kind.asset() => {
                        match (&b.map, b.prop.as_ref().and_then(|p| self.props.get(p))) {
                            (Some(map), _) => map.iter().for_each(|(k, v)| match v {
                                Param::Text(id) => self.texture(&format!("{path}/map/{k}"), id, lut),
                                _ => self.issues.push(Issue::Type(format!("{path}/map/{k}"))),
                            }),
                            (None, Some(p)) if def.kind != ParamKind::Lut && p.kind() == "file" => {}
                            _ => self.issues.push(Issue::Type(path)),
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn kind(&mut self, p: &str, kind: &Kind, depth: usize) {
        match kind {
            Kind::Group { layers, color, .. } => {
                self.tint(p, color.as_ref());
                self.layers(&format!("{p}/layers"), layers, depth + 1)
            }
            Kind::Image { image: named, size, color, playback }
            | Kind::Video { video: named, size, color, playback } => {
                let field = if matches!(kind, Kind::Image { .. }) { "image" } else { "video" };
                self.named(&format!("{p}/{field}"), named);
                self.size(&format!("{p}/size"), *size);
                self.tint(p, color.as_ref());
                playback.iter().for_each(|b| self.playback(&format!("{p}/playback"), b));
            }
            Kind::Text(t) => self.text(p, t),
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
            Kind::Particles { system, color, instance, render } => {
                let known = self.scene.particles.get(system);
                self.known(&format!("{p}/system"), "particle system", system, known.is_some());
                self.tint(p, color.as_ref());
                if let Some(r) = render {
                    self.uses(true, "particles2");
                    self.render(p, r);
                }
                if let Some(i) = instance {
                    self.instance(&format!("{p}/instance"), i, known);
                }
            }
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
            Kind::Adjustment { size, redraw, .. } => {
                let [w, h] = self.scene.canvas.size.map(|v| 2.0 * v as f32);
                if let Some([x, y]) = size {
                    self.range(&format!("{p}/size/0"), *x, 1.0, w);
                    self.range(&format!("{p}/size/1"), *y, 1.0, h);
                }
                self.uses(size.is_some() || *redraw, "capture");
            }
            Kind::Sound(_) | Kind::Light(_) | Kind::Mesh(_) | Kind::Model(_) => {}
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
        if let Some(texture) = &mask.texture {
            self.named(&format!("{path}/texture"), texture);
            self.moving(texture);
        }
        if let Some(id) = &mask.layer {
            let drawn = self.scene.layer(id).is_some_and(Layer::drawn);
            self.known(&format!("{path}/layer"), "layer", id, drawn);
        }
        if let Some(shape) = &mask.shape {
            self.shape(&format!("{path}/shape"), shape);
        }
        self.range(&format!("{path}/feather"), mask.feather, 0.0, 1e4);
        if let Some(strength) = &mask.strength {
            self.uses(true, "mask_strength");
            self.value(&format!("{path}/strength"), strength);
            if let Value::Fixed(v) = strength {
                self.range(&format!("{path}/strength"), *v, 0.0, 1.0);
            }
        }
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
