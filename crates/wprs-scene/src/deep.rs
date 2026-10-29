use crate::Layer;
use crate::effects::Stage;
use crate::figure::{MATERIALS, Material, MeshLayer, ModelLayer};
use crate::validate::{Check, Issue};
use crate::value::{Param, Value};

const MAX_MATERIALS: usize = 64;
const BUILT: [(&str, &str, f32, f32); 12] = [
    ("pbr", "metallic", 0.0, 1.0),
    ("pbr", "roughness", 0.0, 1.0),
    ("pbr", "normal_scale", 0.0, 10.0),
    ("pbr", "occlusion", 0.0, 1.0),
    ("fur", "layers", 1.0, 32.0),
    ("fur", "length", 0.0, 100.0),
    ("fur", "density", 0.0, 1.0),
    ("foliage", "strength", 0.0, 10.0),
    ("foliage", "speed", 0.0, 10.0),
    ("chroma", "key", 0.0, 1.0),
    ("chroma", "threshold", 0.0, 1.0),
    ("chroma", "softness", 0.0, 1.0),
];

impl<'a> Check<'a> {
    fn model_asset(&mut self, path: &str, id: &str) -> Option<&'a crate::gltf::ModelFile> {
        let scene: &'a crate::Scene = self.scene;
        let asset = scene.assets.get(id);
        self.known(path, "asset", id, asset.is_some());
        if asset.is_some_and(|a| a.extension() != "glb") {
            self.issues.push(Issue::Type(path.into()));
        }
        scene.models.get(id).map(|m| &**m)
    }

    pub(crate) fn mesh(&mut self, p: &str, layer: &Layer, m: &MeshLayer) {
        self.uses(true, "mesh");
        let model = self.model_asset(&format!("{p}/mesh"), &m.mesh);
        if let Some(t) = &m.texture {
            self.named(&format!("{p}/texture"), t);
        }
        m.size
            .iter()
            .flatten()
            .enumerate()
            .for_each(|(i, v)| self.range(&format!("{p}/size/{i}"), *v, 1e-3, 1e5));
        m.color.iter().for_each(|c| self.value(&format!("{p}/color"), c));
        self.count(&format!("{p}/parts"), m.parts.len(), crate::gltf::MAX_MORPHS);
        for (name, part) in &m.parts {
            let q = format!("{p}/parts/{name}");
            if model.is_some_and(|f| !f.parts.contains(name)) {
                self.issues.push(Issue::Missing(q.clone(), name.clone()));
            }
            part.order.iter().for_each(|o| self.number(&format!("{q}/order"), o, -1000.0, 1000.0));
            part.visible.iter().for_each(|v| self.value(&format!("{q}/visible"), v));
            if let Some(to) = &part.clip_to {
                self.known(
                    &format!("{q}/clip_to"),
                    "part",
                    to,
                    to != name && model.is_none_or(|f| f.parts.contains(to)),
                );
            }
        }
        if let Some(d) = m.depth {
            self.range(&format!("{p}/depth/scale"), d.scale, 0.0, 10.0);
        }
        self.rig(p, &layer.id, &m.rig, model);
    }

    pub(crate) fn model(&mut self, p: &str, layer: &Layer, m: &ModelLayer) {
        self.uses(true, "model3d");
        let model = self.model_asset(&format!("{p}/model"), &m.model);
        m.size
            .iter()
            .flatten()
            .enumerate()
            .for_each(|(i, v)| self.range(&format!("{p}/size/{i}"), *v, 1e-3, 1e5));
        for (name, v) in [("position", &m.position), ("rotation", &m.rotation), ("scale", &m.scale)] {
            v.iter().for_each(|v| self.value(&format!("{p}/{name}"), v));
        }
        m.env.iter().for_each(|e| self.picture(&format!("{p}/env"), e));
        self.count(&format!("{p}/materials"), m.materials.len(), MAX_MATERIALS);
        for (name, mat) in &m.materials {
            let q = format!("{p}/materials/{name}");
            if model.is_some_and(|f| !f.materials.contains(name)) {
                self.issues.push(Issue::Missing(q.clone(), name.clone()));
            }
            self.material(&q, mat);
        }
        if m.root_motion && m.rig.clips.is_empty() {
            self.issues.push(Issue::Type(format!("{p}/root_motion")));
        }
        self.rig(p, &layer.id, &m.rig, model);
    }

    fn material(&mut self, q: &str, mat: &Material) {
        mat.color.iter().for_each(|c| self.value(&format!("{q}/color"), c));
        mat.emissive.iter().for_each(|e| self.value(&format!("{q}/emissive"), e));
        self.uses(mat.cast_shadows == Some(true) || mat.receive_shadows == Some(true), "shadows");
        let shader = mat.shader.as_deref().unwrap_or("pbr");
        let built = MATERIALS.contains(&shader);
        self.uses(!matches!(shader, "pbr" | "unlit"), "material_shaders");
        if !built {
            self.staged(&format!("{q}/shader"), shader, Stage::Material);
        }
        for (id, value) in &mat.params {
            let path = format!("{q}/params/{id}");
            self.value(&path, value);
            let declared = match built {
                true => {
                    BUILT.iter().find(|(s, n, ..)| *s == shader && n == id).map(|(.., lo, hi)| (*lo, *hi))
                }
                false => self
                    .scene
                    .effect(shader)
                    .and_then(|d| d.params.iter().find(|p| &p.id == id))
                    .map(|d| (d.min.unwrap_or(f32::MIN), d.max.unwrap_or(f32::MAX))),
            };
            let unloaded = !built && self.scene.effect(shader).is_none();
            match (declared, value) {
                (None, _) if !unloaded => self.known(&path, "parameter", id, false),
                (Some((lo, hi)), Value::Fixed(Param::Number(n))) => self.range(&path, *n, lo, hi),
                _ => {}
            }
        }
    }
}
