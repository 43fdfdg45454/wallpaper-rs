use super::super::{Image, Pixels};
use super::Skin;
use crate::Renderer;
use crate::texture::GpuImage;
use std::collections::HashMap;
use std::sync::Arc;

const SHRUNK: f32 = 0.5625;

fn shrunk(image: &GpuImage, (w, h): (u32, u32)) -> bool {
    let (tw, th) = image.size();
    image.texture.mip_level_count() == 1 && (w as f32 * h as f32) < SHRUNK * tw as f32 * th as f32
}

impl Renderer {
    pub(super) fn mip_skin(
        &self,
        (images, sources): (&mut HashMap<String, Image>, &HashMap<String, Arc<Pixels>>),
        skin: &mut Skin,
        named: Option<&str>,
        fits: (u32, u32),
    ) {
        if let Some(id) = named.filter(|id| images.contains_key(*id)) {
            if let Some(image) = images.get(id).filter(|i| shrunk(&i.0, fits)) {
                let mipped = self.mipped(image, sources.get(id));
                images.insert(id.to_owned(), mipped);
            }
            return;
        }
        if let Some(own) = skin.own.as_ref().filter(|own| shrunk(own, fits)) {
            let (w, h) = own.size();
            skin.own = Some(self.mipped(&Arc::new((own.clone(), [w as f32, h as f32])), None).0.clone());
        }
    }
}
