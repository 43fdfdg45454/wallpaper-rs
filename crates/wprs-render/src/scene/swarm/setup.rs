use super::binds::layouts;
use super::kit::kit;
use super::{B, Herd, SIM, Swarms, buffer, compute, mode, module};
use crate::Renderer;
use crate::scene::{Image, SceneAssets};
use std::collections::HashMap;
use wprs_scene::{
    Bake, Blend, EmitShape, Kind, Placement, Quality, Render, Scene, Surface, System, Values, flocks,
};

const PLAIN: &str = "fn particle(p: Sprite) -> Sprite {\n    return p;\n}\n";

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

fn masks(s: &System) -> impl Iterator<Item = &str> {
    let shapes = s.emitters.iter().filter_map(|e| match &e.shape {
        EmitShape::Mask { texture, .. } => Some(texture.as_str()),
        _ => None,
    });
    let walls = s.collide.iter().filter_map(|c| match &c.bounds {
        Surface::Mask { texture, .. } => Some(texture.as_str()),
        _ => None,
    });
    shapes.chain(walls)
}

impl Swarms {
    pub(crate) fn uses(&self, scene: &Scene, id: &str) -> bool {
        self.sprites.iter().flatten().any(|s| s == id)
            || scene.particles.values().flat_map(masks).any(|m| m == id)
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
        let layouts = layouts(r);
        debug_assert!(size_of::<wprs_scene::Sim>() as u64 <= SIM);
        let masks = scene.particles.values().flat_map(masks);
        let masks = masks.filter_map(|t| Some((t, assets.images.get(t)?.image()?)));
        let masks: HashMap<&str, std::sync::Arc<image::RgbaImage>> = masks.collect();
        let cells = crate::scene::sampling::samplings(scene, assets);
        let pixels = |id: &str| masks.get(id).map(|i| (i.width(), i.height(), i.as_raw().as_slice()));
        let sprites = sprites(scene, props);
        let kits = renders(scene).into_iter().zip(&sprites).map(|((system, layer, render), sprite)| {
            let frames = sprite.as_ref().and_then(|t| cells.get(t)).map_or(&[][..], |c| &c.cells[..]);
            let sprite = sprite.as_deref().map(|t| (t, images.get(t)));
            kit(r, &layouts[2], (system, layer), render, scene, (sprite, frames), white, srgb)
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
            kits: kits.collect(),
            herds: herds.collect(),
            sprites,
            flights: Vec::new(),
            marks: Vec::new(),
            stamp: (None, false),
            module: module(
                r,
                &[wprs_scene::STAGES_WGSL, PLAIN, include_str!("sprite.wgsl"), include_str!("ribbon.wgsl")]
                    .join("\n"),
            ),
            layouts,
            compute: None,
            pipelines: HashMap::new(),
            ribbons: HashMap::new(),
            sorts: None,
            format,
            sampler: r.device.create_sampler(&sampler),
            looks: Vec::new(),
            kitted: Vec::new(),
            looked: None,
            bytes: Vec::new(),
            quality: Quality::Q3,
            none,
            maps: Vec::new(),
            stages: HashMap::new(),
            framing: super::stage::framing(r),
            framed: None,
        };
        swarms.maps = Swarms::charts(r, scene, &swarms.herds);
        swarms.prepare(r, gpu);
        drawn.into_iter().chain([&Blend::Normal]).for_each(|b| swarms.pipeline(r, mode(*b, false)));
        Some(swarms)
    }
}
