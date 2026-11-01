use super::kit::Kit;
use super::{PARTICLE, Swarms, binds};
use crate::Renderer;
use crate::scene::pipes::Mode;
use wprs_scene::{Scene, Stage};

const OWN: [&str; 12] = [
    "Look",
    "look",
    "Corner",
    "samp",
    "sprite",
    "rects",
    "curve",
    "decode",
    "encode",
    "cell",
    "vs_particle",
    "fs_particle",
];

fn prefixed(src: &str) -> String {
    let (mut out, mut word) = (String::with_capacity(src.len() + 512), String::new());
    for c in src.chars().chain([' ']) {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if OWN.contains(&word.as_str()) {
            out.push_str("wprs_");
        }
        out.push_str(&word);
        word.clear();
        out.push(c);
    }
    out.pop();
    out
}

pub(super) fn stage(r: &Renderer, scene: &Scene, kit: &mut Kit, id: Option<&str>) {
    let Some(id) = id else { return };
    let Some(d) = scene.loaded.get(id).filter(|d| d.stage == Stage::Particle) else {
        kit.refused = Some((id.into(), "it is not a particle shader".into()));
        return;
    };
    match wprs_scene::particle_shader(d, &prefixed(include_str!("sprite.wgsl"))) {
        Ok(module) => {
            let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
            let label = Some("wprs-scene particle shader");
            kit.stage =
                Some((d.key, r.device.create_shader_module(wgpu::ShaderModuleDescriptor { label, source })));
        }
        Err(why) => kit.refused = Some((id.into(), why)),
    }
}

impl Swarms {
    pub(super) fn staged(&mut self, r: &Renderer, mode: Mode) {
        let sprites = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4];
        for k in 0..self.kits.len() {
            let Some((key, module)) = self.kits[k].stage.as_ref() else { continue };
            if self.stages.contains_key(&(mode, *key)) {
                continue;
            }
            let layouts = [&self.layouts[1], &self.layouts[2], &self.framing];
            let entries = ("wprs_vs_particle", "wprs_fs_particle");
            let pipeline = self.build(r, mode, (module, entries), &layouts, &[(PARTICLE, &sprites)]);
            self.stages.insert((mode, *key), pipeline);
            self.created += 1;
        }
    }

    pub(super) fn framed(&mut self, r: &Renderer, uniform: &wgpu::Buffer) {
        if self.framed.is_some() || self.kits.iter().all(|k| k.stage.is_none()) {
            return;
        }
        let entries = [wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }];
        let desc = wgpu::BindGroupDescriptor { label: None, layout: &self.framing, entries: &entries };
        self.framed = Some(r.device.create_bind_group(&desc));
    }
}

pub(super) fn framing(r: &Renderer) -> wgpu::BindGroupLayout {
    let entries = [binds::entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, binds::plain(0))];
    let label = Some("wprs-scene particle frame");
    r.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label, entries: &entries })
}
