use super::pipes::Pipes;
use super::pool::{Pool, Tex};
use super::state::volume;
use super::{Image, SceneAssets, SceneStats, SceneView, item::FrameData, swarm};
use crate::{RenderError, Renderer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wgpu::{TextureFormat as Format, TextureUsages as Usage};
use wprs_scene::{BlendSpace, Lut, ParamKind, Scene, Values, descriptor};

fn linear(scene: &Scene, id: &str) -> bool {
    scene.assets.get(id).is_some_and(|a| a.linear)
}

fn mips(scene: &Scene, id: &str) -> Option<bool> {
    scene.assets.get(id).and_then(|a| a.mipmaps)
}

impl Renderer {
    pub(crate) fn pipes(&self) -> &Pipes {
        self.scene.get_or_init(|| Pipes::new(&self.device))
    }

    pub fn refresh_scene(
        &self,
        view: &mut SceneView,
        (scene, props): (&Scene, &Values),
        assets: &SceneAssets,
    ) -> Result<bool, RenderError> {
        view.marks.clear();
        view.pool.glow.baked = None;
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
                let upload = |p| self.upload_pixels(p, linear(scene, id), mips(scene, id));
                let found = assets.images.get(id).and_then(|p| Some((p, upload(p)?)));
                view.images.remove(id);
                view.sources.remove(id);
                view.covers.remove(id);
                if let Some((pixels, image)) = found {
                    view.images.insert(id.clone(), image);
                    view.sources.insert(id.clone(), pixels.clone());
                }
            }
            view.samplings = super::sampling::samplings(scene, assets);
            self.mipmap(&mut view.images, &view.samplings, &view.sources);
            let pool = &mut view.pool;
            view.caches.drain().for_each(|(_, c)| pool.give(c.tex));
            if particles {
                let looks = (view.format, view.srgb);
                view.swarms =
                    swarm::Swarms::new(self, (scene, props), assets, &view.images, pool.lamped(), looks);
            }
            changed.iter().filter_map(|id| assets.images.get(id)).for_each(|p| p.release());
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
        let id = super::stops::view();
        let mut cut = std::collections::BTreeMap::new();
        let (ledger, trimmed) = super::limit::Ledger::new(scene, self.tier(), &mut cut);
        let built = trimmed.as_ref().unwrap_or(scene);
        self.scoped(|| {
            let mut lease = self.pipes().prepare(&self.device, built, format, state_format);
            let mut refused = self.own_pipes(built, [format, state_format], &mut lease, id);
            refused.extend(cut);
            self.pipes().warm(&self.device, built, format, srgb);
            let upload = |(id, p): (&String, &Arc<_>)| {
                Some((id.clone(), self.upload_pixels(p, linear(scene, id), mips(scene, id))?))
            };
            let mut images: HashMap<String, Image> = assets.images.iter().filter_map(upload).collect();
            let mut used = HashSet::new();
            for e in scene.drawable().flat_map(|l| &l.effects) {
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
            let mut pool = Pool::new(&self.device, &self.queue, format);
            pool.ledger = ledger;
            let samplings = super::sampling::samplings(scene, assets);
            let sources = assets.images.clone();
            self.mipmap(&mut images, &samplings, &sources);
            let swarms =
                swarm::Swarms::new(self, (scene, props), assets, &images, pool.lamped(), (format, srgb));
            assets.images.values().for_each(|p| p.release());
            SceneView {
                fit: scene.canvas.fit,
                canvas: scene.canvas.size,
                srgb,
                format,
                images,
                samplings,
                sources,
                fonts: super::fonts::Fonts::new(scene, &assets.fonts),
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
                refused,
                scratch: Default::default(),
                marks: Vec::new(),
                stamp: 0,
                trimmed: false,
                region: None,
                shrink: (1.0, 0),
                id,
                paints: self.paints(scene),
                clock: super::clock::Clock::new(self),
                hitboxes: Default::default(),
                covers: HashMap::new(),
                molds: HashMap::new(),
                skins: HashMap::new(),
            }
        })
        .inspect_err(|_| super::flight::forget(id))
    }
}
