use crate::look::Playback;
use crate::validate::{Check, Issue};
use crate::value::Value;
use crate::{Color, Kind, Layer, MAX_FRAMES};

impl Check<'_> {
    pub(crate) fn fields(&mut self, p: &str, layer: &Layer) {
        self.uses(layer.blend.newer(), "blend_modes2");
        self.uses(layer.parallax.apart(), "parallax2");
        if let Some(b) = &layer.brightness {
            self.uses(true, "layer_color");
            self.number(&format!("{p}/brightness"), b, 0.0, 16.0);
        }
        let t = &layer.transform;
        if let Some(tilt) = &t.tilt {
            self.value(&format!("{p}/transform/tilt"), tilt);
            if let Value::Fixed(v) = tilt {
                v.0.iter()
                    .enumerate()
                    .for_each(|(i, a)| self.range(&format!("{p}/transform/tilt/{i}"), *a, -89.0, 89.0));
            }
        }
        if let Some(z) = &t.z {
            self.number(&format!("{p}/transform/z"), z, -1e5, 1e5);
        }
        self.uses(t.tilt.is_some() || t.z.is_some(), "tilt");
        self.uses(layer.parent.is_some() || layer.inherit.is_some(), "parent");
        if let Some(inherit) = &layer.inherit {
            let repeated = inherit.iter().enumerate().any(|(i, k)| inherit[..i].contains(k));
            if layer.parent.is_none() || repeated {
                self.issues.push(Issue::Type(format!("{p}/inherit")));
            }
        }
        let Some(l) = &layer.lighting else { return };
        self.uses(true, "lighting2d");
        let lit = matches!(
            layer.kind,
            Kind::Image { .. } | Kind::Video { .. } | Kind::Text(_) | Kind::Particles { .. }
        );
        if !lit {
            self.issues.push(Issue::Type(format!("{p}/lighting")));
        }
        for (name, id) in [("normal", &l.normal), ("env", &l.env)] {
            id.iter().for_each(|id| self.picture(&format!("{p}/lighting/{name}"), id));
        }
        let parts = [("roughness", &l.roughness, 1.0), ("reflection", &l.reflection, 1.0)];
        let more = [("emissive", &l.emissive, 16.0), ("strength", &l.strength, 1.0)];
        for (name, v, max) in parts.into_iter().chain(more) {
            self.number(&format!("{p}/lighting/{name}"), v, 0.0, max);
        }
    }

    pub(crate) fn number(&mut self, path: &str, v: &Value<f32>, min: f32, max: f32) {
        self.value(path, v);
        if let Value::Fixed(v) = v {
            self.range(path, *v, min, max);
        }
    }

    pub(crate) fn picture(&mut self, path: &str, id: &str) {
        let asset = self.scene.assets.get(id);
        self.known(path, "asset", id, asset.is_some());
        if asset.is_some_and(|a| !crate::stage::IMAGES.contains(&a.extension().as_str())) {
            self.issues.push(Issue::Type(path.into()));
        }
    }

    pub(crate) fn tint(&mut self, p: &str, color: Option<&Value<Color>>) {
        if let Some(c) = color {
            self.uses(true, "layer_color");
            self.value(&format!("{p}/color"), c);
        }
    }

    pub(crate) fn playback(&mut self, p: &str, b: &Playback) {
        self.uses(true, "layer_playback");
        self.number(&format!("{p}/rate"), &b.rate, 0.0, 16.0);
        self.value(&format!("{p}/playing"), &b.playing);
        self.range(&format!("{p}/start"), b.start, 0.0, 86_400.0);
        b.end.iter().for_each(|end| self.range(&format!("{p}/end"), *end, b.start.next_up(), 86_400.0));
        b.frame.iter().for_each(|f| self.number(&format!("{p}/frame"), f, 0.0, (MAX_FRAMES - 1) as f32));
    }
}
