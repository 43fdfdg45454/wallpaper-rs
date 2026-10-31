use crate::validate::{Check, Issue};
use crate::value::Value;
use crate::{Layer, Mask, Shape};
use std::collections::HashMap;

impl Check<'_> {
    pub(super) fn shape(&mut self, path: &str, shape: &Shape) {
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

    pub(super) fn mask(&mut self, path: &str, mask: &Mask) {
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
