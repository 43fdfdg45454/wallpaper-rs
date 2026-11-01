use super::kit::Kit;
use super::sprite::pipe;
use super::staging::{left, within};
use super::{PARTICLE, Swarms, binds};
use crate::Renderer;
use crate::scene::pipes::Mode;
use wprs_scene::{Scene, Stage};

const ENTRIES: (&str, &str) = ("wprs_vs_particle", "wprs_fs_particle");

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
    let made = left(d.key).map_or(Ok(()), Err).and_then(|()| {
        let module = wprs_scene::particle_shader(d, &prefixed(include_str!("sprite.wgsl")))?;
        within((d.key, &r.device), move |device| {
            let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
            let label = Some("wprs-scene particle shader");
            device.create_shader_module(wgpu::ShaderModuleDescriptor { label, source })
        })
    });
    match made {
        Ok(module) => kit.stage = Some((d.key, module)),
        Err(why) => kit.refused = Some((id.into(), why)),
    }
}

pub(super) fn leave(kits: &mut [Kit], key: u64, why: &str) {
    for kit in kits.iter_mut().filter(|k| k.stage.as_ref().is_some_and(|s| s.0 == key)) {
        tracing::warn!("the particle shader of {} was left out: {why}", kit.id);
        kit.stage = None;
        kit.refused = Some((kit.shader.clone().unwrap_or_default(), why.to_owned()));
    }
}

impl Swarms {
    pub(super) fn staged(&mut self, r: &Renderer, mode: Mode) {
        for k in 0..self.kits.len() {
            let Some((key, module)) = self.kits[k].stage.clone() else { continue };
            if self.stages.contains_key(&(mode, key)) {
                continue;
            }
            let groups = [self.layouts[1].clone(), self.layouts[2].clone(), self.framing.clone()];
            let format = self.format;
            let made = within((key, &r.device), move |device| {
                let sprites = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4];
                let (shader, layouts) = ((&module, ENTRIES), [&groups[0], &groups[1], &groups[2]]);
                pipe(device, (format, mode), shader, &layouts, &[(PARTICLE, &sprites)])
            });
            match made {
                Ok(pipeline) => {
                    self.stages.insert((mode, key), pipeline);
                    self.created += 1;
                }
                Err(why) => leave(&mut self.kits, key, &why),
            }
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
