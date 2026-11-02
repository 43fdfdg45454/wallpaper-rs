use crate::Renderer;
use std::sync::PoisonError;

const OWN: [&str; 46] = [
    "Light",
    "Scene3",
    "Object",
    "Bones",
    "Draw",
    "Shadows",
    "Finish",
    "In",
    "Bound",
    "Out",
    "Placed",
    "scene",
    "env",
    "env_smp",
    "shadow_a",
    "shadow_b",
    "shadow_cmp",
    "shade",
    "object",
    "bones",
    "draw",
    "morphs",
    "base_map",
    "orm_map",
    "normal_map",
    "occlusion_map",
    "emissive_map",
    "smp",
    "finish",
    "PI",
    "place",
    "skin",
    "delta",
    "out",
    "posed",
    "vs_solid",
    "vs_skinned",
    "vs_shadow",
    "vs_shadow_skinned",
    "ggx",
    "smith",
    "equirect",
    "falloff",
    "tap",
    "shadowed",
    "hashed",
];
const MORE: [&str; 3] = ["fs_solid", "shown", "lit"];

fn prefixed(src: &str) -> String {
    let (mut out, mut word) = (String::with_capacity(src.len() + 1024), String::new());
    for c in src.chars().chain([' ']) {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if OWN.contains(&word.as_str()) || MORE.contains(&word.as_str()) {
            out.push_str("wprs_");
        }
        out.push_str(&word);
        word.clear();
        out.push(c);
    }
    out.pop();
    out
}

pub(crate) fn solid_source() -> String {
    let own = prefixed(&super::SOURCE.concat());
    format!(
        "@group(0) @binding(3) var<uniform> frame: Frame;\nfn param(i: u32) -> vec4<f32> {{\n    return wprs_finish.params[min(i, 7u)];\n}}\n{own}"
    )
}

impl Renderer {
    pub(super) fn packages(&self, scene: &wprs_scene::Scene, keys: impl Iterator<Item = u64>) {
        let solids = self.solids();
        for key in keys {
            if solids.packages.lock().unwrap_or_else(PoisonError::into_inner).contains_key(&key) {
                continue;
            }
            let found =
                scene.loaded.values().find(|d| d.key == key && d.stage == wprs_scene::Stage::Material);
            let made = found.ok_or_else(|| "it is not a material shader".to_owned()).and_then(|d| {
                let module = wprs_scene::material_shader(d, &solid_source())?;
                let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
                let label = Some("wprs-scene material shader");
                Ok(self.device.create_shader_module(wgpu::ShaderModuleDescriptor { label, source }))
            });
            if let Err(why) = &made {
                tracing::warn!("a material shader was left out: {why}");
            }
            solids.packages.lock().unwrap_or_else(PoisonError::into_inner).insert(key, made.ok());
        }
    }
}
