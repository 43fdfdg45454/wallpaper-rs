use super::build::own;
use super::chains::lock;
use super::pipes::Pipes;
use wgpu::TextureFormat;
use wprs_scene::{Descriptor, Output, PassDef, Texel};

pub(crate) const UNREADY: &str = "its shader was not built for these option values";

pub(crate) fn format(d: &Descriptor, pass: &PassDef, formats: [TextureFormat; 2]) -> TextureFormat {
    let target = pass.target.as_ref().and_then(|t| d.targets.get(t));
    match (target, pass.output) {
        (Some(t), _) if t.format == Texel::Rgba16f => formats[1],
        (Some(_), _) => TextureFormat::Rgba8Unorm,
        (None, Output::State) => formats[1],
        (None, Output::Next) => formats[0],
    }
}

impl Pipes {
    pub(crate) fn ready(&self, d: &Descriptor, variant: u32, formats: [TextureFormat; 2]) -> bool {
        let shelf = lock(&self.shelf);
        let drawn = d.passes.iter().enumerate().filter(|(_, p)| p.copy.is_none() && p.swap.is_none());
        drawn.into_iter().all(|(n, p)| {
            let key = (own(d.key, n, variant), super::own::mode(p.blend), format(d, p, formats));
            shelf.cache.contains_key(&key)
        })
    }
}
