use super::kit::{Look, kit};
use super::{B, Herd, SIM, Swarms, buffer, compute, entry, mode, module, storage, uniform};
use crate::Renderer;
use crate::scene::{Image, SceneAssets};
use std::collections::HashMap;
use wgpu::ShaderStages as S;
use wprs_scene::{Bake, Blend, EmitShape, Kind, Placement, Quality, Render, Scene, Values, flocks};

pub(crate) fn renders(scene: &Scene) -> Vec<(usize, Option<String>, &Render)> {
    let own = scene.layers().filter_map(|l| match &l.kind {
        Kind::Particles { system, render: Some(r), .. } => {
            Some((scene.particles.keys().position(|k| k == system)?, Some(l.id.clone()), &**r))
        }
        _ => None,
    });
    scene.particles.values().enumerate().map(|(i, s)| (i, None, &s.render)).chain(own).collect()
}

pub(crate) fn sprites(scene: &Scene, props: &Values) -> Vec<Option<String>> {
    renders(scene)
        .into_iter()
        .map(|(_, _, r)| r.texture.as_ref().and_then(|t| t.settled(Some(props))))
        .collect()
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
        let (tier, gpu) = (r.tier(), compute(r) && !r.software());
        let layout = |entries: &[wgpu::BindGroupLayoutEntry]| {
            let label = Some("wprs-scene particles");
            r.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label, entries })
        };
        let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let sample_type = wgpu::TextureSampleType::Float { filterable: true };
        let (view_dimension, c) = (wgpu::TextureViewDimension::D2, S::COMPUTE);
        let texture = wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false };
        let layouts = [
            layout(&[
                entry(0, c, storage(false)),
                entry(1, c, storage(false)),
                entry(2, c, storage(true)),
                entry(3, c, storage(true)),
                entry(4, c, uniform(size_of::<wprs_scene::Sim>())),
            ]),
            layout(&[
                entry(0, S::VERTEX_FRAGMENT, uniform(size_of::<Look>())),
                entry(1, S::FRAGMENT, filtering),
            ]),
            layout(&[entry(0, S::FRAGMENT, texture)]),
        ];
        debug_assert!(size_of::<wprs_scene::Sim>() as u64 <= SIM);
        let masks = scene.particles.values().flat_map(|s| &s.emitters).filter_map(|e| match &e.shape {
            EmitShape::Mask { texture, .. } => Some((texture.as_str(), assets.images.get(texture)?.image()?)),
            _ => None,
        });
        let masks: HashMap<&str, std::sync::Arc<image::RgbaImage>> = masks.collect();
        let pixels = |id: &str| masks.get(id).map(|i| (i.width(), i.height(), i.as_raw().as_slice()));
        let sprites = sprites(scene, props);
        let kits = renders(scene).into_iter().zip(&sprites).map(|((system, layer, render), sprite)| {
            let sprite = sprite.as_deref().map(|t| (t, images.get(t)));
            kit(r, &layouts[2], (system, layer), render, scene, sprite, white, srgb)
        });
        let systems: Vec<_> = scene.particles.values().collect();
        let herds = flocks(scene).into_iter().enumerate().filter_map(|(i, f)| {
            let world = systems[f.system].space == Placement::World;
            Some(Herd { bake: Bake::new(scene, i, tier, !gpu, pixels)?, world })
        });
        let filter = wgpu::FilterMode::Linear;
        let sampler =
            wgpu::SamplerDescriptor { mag_filter: filter, min_filter: filter, ..Default::default() };
        let none = buffer(r, 16, B::STORAGE);
        let mut swarms = Swarms {
            canvas: scene.canvas.size.map(|v| v as f32),
            steps: 0,
            micros: 0,
            created: 0,
            gpu,
            profile: false,
            trail: tier.trail(),
            kits: kits.collect(),
            herds: herds.collect(),
            sprites,
            flights: Vec::new(),
            marks: Vec::new(),
            module: module(r, include_str!("sprite.wgsl")),
            layouts,
            compute: None,
            pipelines: HashMap::new(),
            format,
            sampler: r.device.create_sampler(&sampler),
            looks: Vec::new(),
            kitted: Vec::new(),
            looked: None,
            bytes: Vec::new(),
            quality: Quality::Q3,
            none,
        };
        swarms.prepare(r, gpu);
        drawn.into_iter().chain([&Blend::Normal]).for_each(|b| swarms.pipeline(r, mode(*b, false)));
        Some(swarms)
    }
}
