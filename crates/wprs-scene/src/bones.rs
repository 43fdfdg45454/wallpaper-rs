use crate::gltf::ModelFile;
use crate::rig::{
    MAX_ANIMATION_LAYERS, MAX_BODIES, MAX_CHAIN, MAX_CHAINS, MAX_CLIPS_PER_MODEL, MAX_EXPRESSIONS, Reach, Rig,
};
use crate::validate::{Check, Issue};
use std::collections::BTreeSet;

mod tracks;

impl Check<'_> {
    pub(crate) fn rig(&mut self, p: &str, layer: &str, rig: &Rig, model: Option<&ModelFile>) {
        let bone = |c: &mut Self, path: String, name: &str| {
            if model.is_some_and(|m| !m.bones.contains(name)) {
                c.issues.push(Issue::Missing(path, name.into()));
            }
        };
        self.count(&format!("{p}/clips"), rig.clips.len(), MAX_CLIPS_PER_MODEL);
        let mut ids = BTreeSet::new();
        for (i, clip) in rig.clips.iter().enumerate() {
            let q = format!("{p}/clips/{i}");
            self.id(&format!("{q}/id"), &clip.id);
            if !ids.insert(clip.id.as_str()) {
                self.issues.push(Issue::DuplicateId(format!("{q}/id")));
            }
            let length = match (model, &clip.animation) {
                (Some(m), Some(name)) => m.animations.get(name).copied().or_else(|| {
                    self.issues.push(Issue::Missing(format!("{q}/animation"), name.clone()));
                    None
                }),
                (_, None) if clip.tracked() => Some(clip.end()),
                (Some(m), None) => m.animations.values().next().copied().or_else(|| {
                    self.issues.push(Issue::Missing(format!("{q}/animation"), "any animation".into()));
                    None
                }),
                (None, _) => None,
            };
            let end = length.unwrap_or(86_400.0);
            self.range(&format!("{q}/from"), clip.from, 0.0, end);
            let to = clip.to.unwrap_or(end);
            clip.to.iter().for_each(|t| self.range(&format!("{q}/to"), *t, clip.from.next_up(), end));
            self.tracks(&q, clip, model);
            self.count(&format!("{q}/events"), clip.events.len(), crate::MAX_EVENTS);
            for (k, e) in clip.events.iter().enumerate() {
                self.range(&format!("{q}/events/{k}/t"), e.t, 0.0, (to - clip.from).max(0.0));
                self.id(&format!("{q}/events/{k}/name"), &e.name);
            }
        }
        self.count(&format!("{p}/animations"), rig.animations.len(), MAX_ANIMATION_LAYERS);
        for (i, a) in rig.animations.iter().enumerate() {
            let q = format!("{p}/animations/{i}");
            self.known(&format!("{q}/clip"), "clip", &a.clip, ids.contains(a.clip.as_str()));
            self.number(&format!("{q}/weight"), &a.weight, 0.0, 1.0);
            self.number(&format!("{q}/rate"), &a.rate, 0.0, 16.0);
            self.range(&format!("{q}/blend_in"), a.blend_in, 0.0, 10.0);
            self.range(&format!("{q}/blend_out"), a.blend_out, 0.0, 10.0);
            self.value(&format!("{q}/enabled"), &a.enabled);
            a.bones.iter().enumerate().for_each(|(k, b)| bone(self, format!("{q}/bones/{k}"), b));
        }
        self.count(&format!("{p}/ik"), rig.ik.len(), MAX_CHAINS);
        self.uses(!rig.ik.is_empty(), "ik");
        for (i, ik) in rig.ik.iter().enumerate() {
            let q = format!("{p}/ik/{i}");
            self.range(&format!("{q}/chain"), ik.chain.len() as f32, 2.0, MAX_CHAIN as f32);
            ik.chain.iter().enumerate().for_each(|(k, b)| bone(self, format!("{q}/chain/{k}"), b));
            if ik.chain.iter().enumerate().any(|(k, b)| ik.chain[..k].contains(b)) {
                self.issues.push(Issue::DuplicateId(format!("{q}/chain")));
            }
            let hangs = |w: &[String]| model.is_none_or(|m| m.parents.get(&w[1]) == Some(&w[0]));
            if !ik.chain.windows(2).all(hangs) {
                self.issues.push(Issue::Unlinked(format!("{q}/chain")));
            }
            match &ik.target {
                Reach::Point(v) => self.value(&format!("{q}/target"), v),
                Reach::Named(name) => self.reach(&format!("{q}/target"), name, layer, model),
            }
            for (name, [lo, hi]) in &ik.limits {
                let path = format!("{q}/limits/{name}");
                self.known(&path, "bone of the chain", name, ik.chain.contains(name));
                self.range(&format!("{path}/0"), *lo, -180.0, 180.0);
                self.range(&format!("{path}/1"), *hi, *lo, 180.0);
            }
            self.number(&format!("{q}/weight"), &ik.weight, 0.0, 1.0);
        }
        self.count(&format!("{p}/physics"), rig.physics.len(), MAX_BODIES);
        self.uses(!rig.physics.is_empty(), "bone_physics");
        for (i, b) in rig.physics.iter().enumerate() {
            let q = format!("{p}/physics/{i}");
            bone(self, format!("{q}/bone"), &b.bone);
            if rig.physics[..i].iter().any(|o| o.bone == b.bone) {
                self.issues.push(Issue::DuplicateId(format!("{q}/bone")));
            }
            for (name, v) in
                [("stiffness", b.stiffness), ("damping", b.damping), ("inertia", b.inertia), ("wind", b.wind)]
            {
                self.range(&format!("{q}/{name}"), v, 0.0, 1.0);
            }
            self.range(&format!("{q}/gravity"), b.gravity, -10.0, 10.0);
            for (name, limit, max) in [("rotation", b.rotation, 180.0), ("translation", b.translation, 1e4)] {
                let Some([lo, hi]) = limit else { continue };
                self.range(&format!("{q}/{name}/0"), lo, -max, max);
                self.range(&format!("{q}/{name}/1"), hi, lo, max);
            }
        }
        self.count(&format!("{p}/rules"), rig.rules.len(), MAX_BODIES);
        self.uses(!rig.rules.is_empty(), "bone_rules");
        for (i, r) in rig.rules.iter().enumerate() {
            let q = format!("{p}/rules/{i}");
            bone(self, format!("{q}/bone"), &r.bone);
            bone(self, format!("{q}/parent"), &r.parent);
            if r.bone == r.parent {
                self.issues.push(Issue::Circle(format!("{q}/parent")));
            }
            self.number(&format!("{q}/weight"), &r.weight, 0.0, 1.0);
            self.ruled(&q, rig, r, model);
        }
        self.shapes(p, rig, model);
        rig.hit.iter().enumerate().for_each(|(k, b)| bone(self, format!("{p}/hit/{k}"), b));
        if rig.hit.iter().enumerate().any(|(k, b)| rig.hit[..k].contains(b)) {
            self.issues.push(Issue::DuplicateId(format!("{p}/hit")));
        }
        self.uses(!rig.hit.is_empty(), "layer_events");
    }

    fn shapes(&mut self, p: &str, rig: &Rig, model: Option<&ModelFile>) {
        self.uses(!rig.shapes.is_empty() || !rig.expressions.is_empty(), "blend_shapes");
        let shape = |c: &mut Self, path: String, name: &str| {
            if model.is_some_and(|m| !m.shapes.contains(name)) {
                c.issues.push(Issue::Missing(path, name.into()));
            }
        };
        self.count(&format!("{p}/expressions"), rig.expressions.len(), MAX_EXPRESSIONS);
        for (name, parts) in &rig.expressions {
            let q = format!("{p}/expressions/{name}");
            self.filled(&q, name);
            if model.is_some_and(|m| m.shapes.contains(name)) {
                self.issues.push(Issue::DuplicateId(q.clone()));
            }
            for (s, w) in parts {
                shape(self, format!("{q}/{s}"), s);
                self.range(&format!("{q}/{s}"), *w, -1.0, 1.0);
            }
        }
        self.count(&format!("{p}/shapes"), rig.shapes.len(), crate::gltf::MAX_MORPHS + MAX_EXPRESSIONS);
        for (name, v) in &rig.shapes {
            let q = format!("{p}/shapes/{name}");
            if !rig.expressions.contains_key(name) {
                shape(self, q.clone(), name);
            }
            self.number(&q, v, 0.0, 1.0);
        }
    }

    fn reach(&mut self, path: &str, name: &str, layer: &str, model: Option<&ModelFile>) {
        match name.split_once(':') {
            None if name == "cursor" => {}
            Some(("bone", b)) if model.is_some_and(|m| !m.bones.contains(b) && !m.nodes.contains(b)) => {
                self.issues.push(Issue::Missing(path.into(), b.into()))
            }
            Some(("bone", _)) => {}
            Some(("layer", id)) => {
                let usable = id != layer && self.scene.layer(id).is_some_and(crate::Layer::drawn);
                self.known(path, "layer", id, usable);
            }
            _ => self.known(path, "target", name, false),
        }
    }
}
