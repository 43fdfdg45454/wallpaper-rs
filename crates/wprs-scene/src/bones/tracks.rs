use crate::gltf::ModelFile;
use crate::rig::AnimClip;
use crate::timeline::Key;
use crate::validate::{Check, Issue};

impl AnimClip {
    pub fn tracked(&self) -> bool {
        !self.bones.is_empty() || !self.shapes.is_empty() || !self.order.is_empty()
    }

    pub fn end(&self) -> f32 {
        let last = |keys: &[Key<f32>]| keys.last().map_or(0.0, |k| k.t);
        let bones = self.bones.values().map(|b| b.last());
        let rest = self.shapes.values().chain(self.order.values()).map(|k| last(k));
        bones.chain(rest).fold(0.0, f32::max)
    }
}

impl Check<'_> {
    pub(super) fn tracks(&mut self, q: &str, clip: &AnimClip, model: Option<&ModelFile>) {
        let name = |c: &mut Self, path: String, name: &str, known: Option<bool>| {
            if known == Some(false) {
                c.issues.push(Issue::Missing(path, name.into()));
            }
        };
        let numbers = |c: &mut Self, path: &str, keys: &[Key<f32>], lo: f32, hi: f32| {
            c.keys_sorted(path, keys.iter().map(|k| k.t));
            keys.iter().enumerate().for_each(|(k, key)| c.range(&format!("{path}/{k}"), key.v, lo, hi));
        };
        self.count(&format!("{q}/bones"), clip.bones.len(), crate::gltf::MAX_BONES);
        for (bone, keys) in &clip.bones {
            let p = format!("{q}/bones/{bone}");
            name(self, p.clone(), bone, model.map(|m| m.nodes.contains(bone)));
            numbers(self, &format!("{p}/rotation"), &keys.rotation, -1e5, 1e5);
            for (field, list, [lo, hi]) in
                [("position", &keys.position, [-1e5, 1e5]), ("scale", &keys.scale, [-1e3, 1e3])]
            {
                let path = format!("{p}/{field}");
                self.keys_sorted(&path, list.iter().map(|k| k.t));
                for (k, key) in list.iter().enumerate() {
                    key.v.0.iter().for_each(|v| self.range(&format!("{path}/{k}"), *v, lo, hi));
                }
            }
        }
        self.count(&format!("{q}/shapes"), clip.shapes.len(), crate::gltf::MAX_MORPHS);
        for (shape, keys) in &clip.shapes {
            let p = format!("{q}/shapes/{shape}");
            name(self, p.clone(), shape, model.map(|m| m.shapes.contains(shape)));
            numbers(self, &p, keys, -1.0, 1.0);
        }
        self.count(&format!("{q}/order"), clip.order.len(), crate::gltf::MAX_MORPHS);
        for (part, keys) in &clip.order {
            let p = format!("{q}/order/{part}");
            name(self, p.clone(), part, model.map(|m| m.parts.contains(part)));
            numbers(self, &p, keys, -1000.0, 1000.0);
        }
    }
}

impl Check<'_> {
    pub(super) fn ruled(
        &mut self,
        q: &str,
        rig: &crate::rig::Rig,
        rule: &crate::rig::BoneRule,
        model: Option<&ModelFile>,
    ) {
        let Some(m) = model else { return };
        let mut stack = vec![rule.parent.as_str()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(at) = stack.pop() {
            if at == rule.bone {
                self.issues.push(Issue::Circle(format!("{q}/parent")));
                return;
            }
            if seen.insert(at) {
                stack.extend(m.parents.get(at).map(String::as_str));
                stack.extend(rig.rules.iter().filter(|r| r.bone == at).map(|r| r.parent.as_str()));
            }
        }
    }
}
