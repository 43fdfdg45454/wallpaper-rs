use crate::stage::{IMAGES, VIDEOS};
use crate::validate::{Check, Issue};
use crate::{Kind, Layer, Value};

fn lit(layer: &Layer, scene: &crate::Scene) -> bool {
    let swarm = match &layer.kind {
        Kind::Particles { system, render, .. } => {
            let own = render.as_deref().or_else(|| scene.particles.get(system).map(|s| &s.render));
            own.is_some_and(|r| r.lit || r.normal.is_some() || r.refract.is_some())
        }
        _ => false,
    };
    layer.lighting.is_some() || swarm
}

pub fn projectable(scene: &crate::Scene, id: &str) -> bool {
    let (mut stack, mut seen) = (vec![id], std::collections::HashSet::new());
    while let Some(at) = stack.pop() {
        let Some(layer) = scene.layer(at).filter(|_| seen.insert(at)) else { continue };
        if lit(layer, scene) {
            return false;
        }
        stack.extend(layer.children().iter().map(|c| c.id.as_str()));
        stack.extend(crate::links::reads(layer));
    }
    scene.layer(id).is_some_and(Layer::drawn)
}

impl Check<'_> {
    pub(crate) fn projected(&mut self, path: &str, texture: &Value<String>) {
        let layer = if let Value::Bound(b) = texture { b.layer.as_deref() } else { None };
        let Some(id) = layer else {
            self.named(path, texture);
            self.moving(texture);
            return self.pictured(path, texture);
        };
        self.uses(true, "layer_textures");
        let scene = self.scene;
        let drawn = scene.layer(id).is_some_and(Layer::drawn);
        self.known(&format!("{path}/layer"), "layer", id, drawn);
        if drawn && !projectable(scene, id) {
            self.issues.push(Issue::Circle(format!("{path}/layer")));
        }
    }

    fn pictured(&mut self, path: &str, texture: &Value<String>) {
        let ids: Vec<&String> = match texture {
            Value::Fixed(id) => vec![id],
            Value::Bound(b) => b.map.iter().flat_map(|m| m.values()).collect(),
        };
        let kinds = |a: &crate::Asset| {
            let ext = a.extension();
            IMAGES.contains(&ext.as_str()) || VIDEOS.contains(&ext.as_str())
        };
        if ids.iter().filter_map(|id| self.scene.assets.get(*id)).any(|a| !kinds(a)) {
            self.issues.push(Issue::Type(path.into()));
        }
    }
}
