use crate::probe::Probe;
use crate::validate::{Check, Issue};
use crate::value::{Value, Vec2};
use crate::{Kind, Layer, MAX_DEPTH};

mod effects;
mod kinds;
mod shapes;

impl Check<'_> {
    fn texture(&mut self, path: &str, id: &str, lut: bool) {
        self.moving(&Value::Fixed(id.into()));
        let cube = self.scene.assets.get(id).map(|a| a.path.to_lowercase().ends_with(".cube"));
        let generic = !lut && crate::GENERIC_PICTURES.contains(&id);
        self.known(path, "asset", id, cube.is_some() || generic);
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
}
