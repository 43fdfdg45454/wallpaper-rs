use super::kit::{Look, kit};
use super::{Swarms, compute, entry, mode, module, storage, uniform};
use crate::Renderer;
use crate::scene::{Image, SceneAssets};
use std::collections::HashMap;
use wgpu::ShaderStages as S;
use wprs_scene::{Bake, Blend, EmitShape, Kind, Quality, Scene, Sim, Tier, Values};

pub(crate) fn sprites(scene: &Scene, props: &Values) -> Vec<Option<String>> {
    scene.particles.values().map(|s| s.render.texture.as_ref().and_then(|t| t.settled(Some(props)))).collect()
}

impl Swarms {
    pub(crate) fn uses(&self, scene: &Scene, id: &str) -> bool {
        let masked = |s: &wprs_scene::System| {
            s.emitters.iter().any(|e| matches!(&e.shape, EmitShape::Mask { texture, .. } if texture == id))
        };
        self.sprites.iter().flatten().any(|s| s == id) || scene.particles.values().any(masked)
    }

    pub(crate) fn new(
        r: &Renderer,
        (scene, props): (&Scene, &Values),
        assets: &SceneAssets,
        images: &HashMap<String, Image>,
        white: &wgpu::TextureView,
        (format, srgb): (wgpu::TextureFormat, bool),
    ) -> Option<Swarms> {
        let drawn: Vec<&Blend> = scene
            .layers()
            .filter_map(|l| matches!(l.kind, Kind::Particles { .. }).then_some(&l.blend))
            .collect();
        if drawn.is_empty() || scene.particles.is_empty() {
            return None;
        }
        let (tier, gpu) = (Tier::detect(r.software(), r.discrete()), compute(r) && !r.software());
        let layout = |entries: &[wgpu::BindGroupLayoutEntry]| {
            let label = Some("wprs-scene particles");
            r.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label, entries })
        };
        let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let sample_type = wgpu::TextureSampleType::Float { filterable: true };
        let (view_dimension, c) = (wgpu::TextureViewDimension::D2, S::COMPUTE);
        let texture = wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false };
        let rw = [0, 1, 2].map(|b| entry(b, c, storage(false)));
        let layouts = [
            layout(&[
                rw[0],
                rw[1],
                rw[2],
                entry(3, c, storage(true)),
                entry(4, c, uniform(size_of::<Sim>())),
            ]),
            layout(&[
                entry(0, S::VERTEX_FRAGMENT, uniform(size_of::<Look>())),
                entry(1, S::FRAGMENT, filtering),
            ]),
            layout(&[entry(0, S::FRAGMENT, texture)]),
        ];
        let pixels = |id: &str| assets.images.get(id).map(|i| (i.width(), i.height(), i.as_raw().as_slice()));
        let sprites = sprites(scene, props);
        let kits = scene.particles.iter().zip(&sprites).filter_map(|((id, s), sprite)| {
            let sprite = sprite.as_deref().map(|t| (t, images.get(t)));
            Some(kit(id, s, Bake::new(scene, id, tier, !gpu, pixels)?, scene, sprite, white, srgb))
        });
        let filter = wgpu::FilterMode::Linear;
        let sampler =
            wgpu::SamplerDescriptor { mag_filter: filter, min_filter: filter, ..Default::default() };
        let mut swarms = Swarms {
            canvas: scene.canvas.size.map(|v| v as f32),
            steps: 0,
            micros: 0,
            created: 0,
            gpu,
            profile: false,
            kits: kits.collect(),
            sprites,
            stores: Vec::new(),
            marks: Vec::new(),
            module: module(r, include_str!("sprite.wgsl")),
            layouts,
            compute: None,
            pipelines: HashMap::new(),
            format,
            sampler: r.device.create_sampler(&sampler),
            looks: Vec::new(),
            looked: None,
            bytes: Vec::new(),
            quality: Quality::Q3,
        };
        swarms.prepare(r, gpu);
        drawn.into_iter().chain([&Blend::Normal]).for_each(|b| swarms.pipeline(r, mode(*b, false)));
        Some(swarms)
    }
}
