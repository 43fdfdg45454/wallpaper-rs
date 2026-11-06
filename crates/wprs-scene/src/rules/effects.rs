use crate::validate::{Check, Issue};
use crate::value::{Binding, Param, Value};
use crate::{Layer, MAX_EFFECTS, ParamDef, ParamKind};

impl Check<'_> {
    pub(super) fn effects(&mut self, p: &str, layer: &Layer) {
        self.count(&format!("{p}/effects"), layer.effects.len(), MAX_EFFECTS);
        for (i, effect) in layer.effects.iter().enumerate() {
            let e = format!("{p}/effects/{i}");
            self.value(&format!("{e}/enabled"), &effect.enabled);
            if let Some(mask) = &effect.mask {
                self.mask(&format!("{e}/mask"), mask);
                self.uses(mask.layer.is_some() || mask.shape.is_some(), "effect_masks");
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
                    Value::Bound(b) if def.kind == ParamKind::Emitters => self.emitted(&path, def, b),
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

    fn emitted(&mut self, path: &str, def: &ParamDef, b: &Binding<Param>) {
        let listed = b.keys.is_some() || (b.prop.is_some() && b.map.is_some());
        let shaped = [b.scale, b.offset, b.exponent].iter().any(Option::is_some) || b.invert;
        let more = b.clamp.is_some() || b.bounds.is_some() || b.smooth.is_some() || b.base.is_some();
        if !listed || shaped || more || b.tracked() {
            self.issues.push(Issue::Type(path.into()));
        }
        let keys = b.keys.iter().flatten().enumerate().map(|(i, k)| (format!("{path}/keys/{i}/1"), &k.v));
        let map = b.map.iter().flatten().map(|(k, v)| (format!("{path}/map/{k}"), v));
        for (at, value) in keys.chain(map) {
            if !def.accepts(value) {
                self.issues.push(Issue::Type(at));
            }
        }
    }
}
