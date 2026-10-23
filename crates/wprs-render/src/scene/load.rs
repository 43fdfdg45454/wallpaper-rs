use super::pipes::Pipes;
use super::pool::{Pool, Tex};
use super::state::volume;
use super::{Image, SceneAssets, SceneStats, SceneView, item::FrameData, swarm, text};
use crate::texture::{gpu_image, texture};
use crate::{RenderError, Renderer};
use image::RgbaImage;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wgpu::{TextureFormat as Format, TextureUsages as Usage};
use wprs_scene::{BlendSpace, Lut, ParamKind, Scene, Values, descriptor};

impl Renderer {
    pub(crate) fn pipes(&self) -> &Pipes {
        self.scene.get_or_init(|| Pipes::new(&self.device))
    }

    fn picture(&self, scene: &Scene, id: &str, image: &RgbaImage) -> Image {
        let linear = scene.assets.get(id).is_some_and(|a| a.linear);
        let prepared = crate::decode::scaled(image, 1.0, self.max_edge());
        let (w, h) = prepared.dimensions();
        let format = if linear { Format::Rgba8Unorm } else { Format::Rgba8UnormSrgb };
        let gpu = gpu_image(texture(self, prepared.as_raw(), (w, h), w * 4, format));
        (gpu, [image.width() as f32, image.height() as f32])
    }

    pub fn refresh_scene(
        &self,
        view: &mut SceneView,
        (scene, props): (&Scene, &Values),
        assets: &SceneAssets,
    ) -> Result<bool, RenderError> {
        let same = |id: &String| match (view.sources.get(id), assets.images.get(id)) {
            (Some(was), Some(now)) => Arc::ptr_eq(was, now),
            (was, now) => was.is_none() && now.is_none(),
        };
        let mut changed: Vec<String> =
            view.sources.keys().chain(assets.images.keys()).filter(|id| !same(id)).cloned().collect();
        changed.sort();
        changed.dedup();
        let sprites = swarm::sprites(scene, props);
        let particles = view
            .swarms
            .as_ref()
            .is_some_and(|s| s.sprites != sprites || changed.iter().any(|id| s.uses(scene, id)));
        if changed.is_empty() && !particles {
            return Ok(false);
        }
        self.scoped(|| {
            for id in &changed {
                let Some(image) = assets.images.get(id) else {
                    view.images.remove(id);
                    view.sources.remove(id);
                    continue;
                };
                let linear = |other: &String| scene.assets.get(other).is_some_and(|a| a.linear);
                let twin =
                    view.sources.iter().find(|(k, p)| Arc::ptr_eq(p, image) && linear(k) == linear(id));
                let shown = twin.and_then(|(k, _)| view.images.get(k).cloned());
                view.images.insert(id.clone(), shown.unwrap_or_else(|| self.picture(scene, id, image)));
                view.sources.insert(id.clone(), image.clone());
            }
            view.caches.clear();
            if particles {
                let looks = (view.format, view.srgb);
                view.swarms = swarm::Swarms::new(
                    self,
                    (scene, props),
                    assets,
                    &view.images,
                    &view.pool.white.view,
                    looks,
                );
            }
        })?;
        Ok(true)
    }

    pub fn scene_view(
        &self,
        scene: &Scene,
        assets: &SceneAssets,
        props: &Values,
    ) -> Result<SceneView, RenderError> {
        let srgb = scene.canvas.blend_space == BlendSpace::Srgb;
        let half = self.gpu.get_texture_format_features(Format::Rgba16Float);
        let flags = wgpu::TextureFormatFeatureFlags::FILTERABLE | wgpu::TextureFormatFeatureFlags::BLENDABLE;
        let usable = half.allowed_usages.contains(Usage::RENDER_ATTACHMENT) && half.flags.contains(flags);
        let format = match (srgb, usable) {
            (true, _) => Format::Rgba8Unorm,
            (false, true) => Format::Rgba16Float,
            (false, false) => Format::Rgba8UnormSrgb,
        };
        let state_format = if usable { Format::Rgba16Float } else { format };
        self.scoped(|| {
            let lease = self.pipes().prepare(&self.device, scene, format, state_format);
            let mut shared: HashMap<(*const RgbaImage, bool), Image> = HashMap::new();
            let upload = |(id, image): (&String, &Arc<RgbaImage>)| {
                let linear = scene.assets.get(id).is_some_and(|a| a.linear);
                let image = shared
                    .entry((Arc::as_ptr(image), linear))
                    .or_insert_with(|| self.picture(scene, id, image));
                (id.clone(), image.clone())
            };
            let mut used = HashSet::new();
            for e in scene.layers().flat_map(|l| &l.effects) {
                let params = descriptor(&e.effect).into_iter().flat_map(|d| &d.params);
                used.extend(params.filter(|p| p.kind == ParamKind::Lut).filter_map(|p| p.named(e)));
            }
            let mut volumes: HashMap<*const Lut, Tex> = HashMap::new();
            let mut volume_of = |(id, lut): (&String, &Arc<Lut>)| {
                let make = || volume(&self.device, &self.queue, lut.size, &lut.data);
                (id.clone(), volumes.entry(Arc::as_ptr(lut)).or_insert_with(make).clone())
            };
            let usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
            let size = std::mem::size_of::<FrameData>() as u64;
            let (images, pool) =
                (assets.images.iter().map(upload).collect(), Pool::new(&self.device, &self.queue, format));
            let swarms =
                swarm::Swarms::new(self, (scene, props), assets, &images, &pool.white.view, (format, srgb));
            let sources = assets.images.clone();
            SceneView {
                fit: scene.canvas.fit,
                canvas: scene.canvas.size,
                srgb,
                format,
                images,
                sources,
                fonts: text::Fonts::new(&assets.fonts),
                texts: HashMap::new(),
                pool,
                caches: HashMap::new(),
                luts: assets
                    .luts
                    .iter()
                    .filter(|(id, _)| used.contains(id.as_str()))
                    .map(&mut volume_of)
                    .collect(),
                states: HashMap::new(),
                state_format,
                target: None,
                uniform: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("wprs-scene frame"),
                    size,
                    usage,
                    mapped_at_creation: false,
                }),
                stats: SceneStats::default(),
                swarms,
                _lease: lease,
            }
        })
    }
}
