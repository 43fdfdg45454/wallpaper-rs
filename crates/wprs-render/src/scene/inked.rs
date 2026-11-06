use super::SceneAssets;
use super::sampling::Sampling;
use std::collections::{HashMap, HashSet};
use wprs_scene::{Scene, Value};

pub(crate) type Inked = [[f32; 4]; 4];

const EMPTY: [f32; 4] = [1.0, 1.0, 0.0, 0.0];

fn bounds(image: &image::RgbaImage) -> Inked {
    let (w, h) = image.dimensions();
    let mut found = [[u32::MAX, u32::MAX, 0, 0]; 4];
    for (x, y, p) in image.enumerate_pixels() {
        for (b, v) in found.iter_mut().zip(p.0) {
            if v > 0 {
                *b = [b[0].min(x), b[1].min(y), b[2].max(x + 1), b[3].max(y + 1)];
            }
        }
    }
    found.map(|[x0, y0, x1, y1]| match x1 > x0 {
        true => [x0 as f32 / w as f32, y0 as f32 / h as f32, x1 as f32 / w as f32, y1 as f32 / h as f32],
        false => EMPTY,
    })
}

pub(crate) fn ink(scene: &Scene, assets: &SceneAssets, samplings: &mut HashMap<String, Sampling>) {
    let uses = || scene.drawable().flat_map(|l| &l.effects);
    let masks = uses().filter_map(|e| match e.mask.as_ref()?.texture.as_ref()? {
        Value::Fixed(id) => Some(id.as_str()),
        Value::Bound(_) => None,
    });
    let limits = uses()
        .filter_map(|e| Some((e, scene.effect(&e.effect)?)))
        .flat_map(|(e, d)| d.bounded.iter().filter_map(move |b| d.params.get(d.param(b)?)?.named(e)));
    let masks = masks.chain(limits);
    for id in masks.collect::<HashSet<_>>() {
        let Some(image) = assets.images.get(id).and_then(|p| p.image()) else { continue };
        samplings.entry(id.to_string()).or_default().inked = Some(bounds(&image));
    }
}
