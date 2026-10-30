use crate::area::covered;
use crate::{Kind, Layer, Quality, Scene, Tier};
use std::collections::HashMap;

const BACKDROP: f32 = 2.0;
const FED: f32 = 1.0;
const APART: f32 = 1.0;
const BASE: f32 = 1.5;

impl Scene {
    pub fn areas(&self) -> HashMap<*const Layer, f32> {
        let view = [0.0, 0.0, self.canvas.size[0] as f32, self.canvas.size[1] as f32];
        self.bounds().into_iter().map(|(l, b)| (std::ptr::from_ref(l), covered(b, view))).collect()
    }

    pub fn heavy(&self) -> Option<String> {
        let (cost, budget) = (self.cost(Quality::Q2), Tier::T1.budget());
        (cost > budget).then(|| {
            format!(
                "the scene costs {cost:.1} units a frame at quality level 2, more than the {budget} that older \
                 integrated and phone GPUs have, so they show it with fewer details"
            )
        })
    }

    fn units(&self, l: &Layer, quality: Quality) -> f32 {
        let effects = l.effects.iter().filter_map(|e| {
            let d = self.effect(&e.effect)?;
            let scale = quality.effect_scale(d.class(), e.optional)? * d.scale_at(quality);
            let scale = if d.passes.len() > 1 { scale } else { 1.0 };
            Some(d.units() * scale * scale)
        });
        let apart = !l.effects.is_empty() || l.mask.is_some() || !l.blend.fixed() || l.cache;
        let below = l.effects.iter().filter_map(|e| self.effect(&e.effect)).map(|d| match d {
            d if d.compose => 2.0 * BACKDROP,
            d if d.below() => BACKDROP,
            _ => 0.0,
        });
        let backdrop = below.sum::<f32>() + if l.blend.fixed() { 0.0 } else { BACKDROP };
        let fed = l.effects.iter().map(|e| {
            let layers = e.params.values().filter(|v| v.binding().is_some_and(|b| b.layer.is_some()));
            let drawn = e.mask.as_ref().is_some_and(|m| m.layer.is_some() || m.shape.is_some());
            (layers.count() + usize::from(drawn)) as f32 * FED
        });
        effects.sum::<f32>() + fed.sum::<f32>() + backdrop + if apart { APART } else { 0.0 }
    }

    pub fn copies_cost<'a>(&self, copies: impl Iterator<Item = &'a Layer>, quality: Quality) -> f32 {
        let mut stack: Vec<&Layer> = copies.collect();
        let mut units = 0.0;
        while let Some(l) = stack.pop() {
            stack.extend(l.children());
            units += if l.flat() { self.units(l, quality) } else { 0.0 };
        }
        let scale = quality.render_scale();
        units * scale * scale
    }

    pub fn cost(&self, quality: Quality) -> f32 {
        let areas = self.areas();
        let area = |l: &Layer| areas.get(&std::ptr::from_ref(l)).copied().unwrap_or(1.0);
        let layers = self.layers().filter(|l| l.flat()).map(|l| self.units(l, quality) * area(l));
        let particles = self.layers().filter_map(|l| match &l.kind {
            Kind::Particles { system, .. } => self.particles.get(system),
            _ => None,
        });
        let canvas = self.canvas.size;
        let particles =
            particles.map(|s| (s.capacity as f32 / 100_000.0 + s.overdraw(canvas)) * quality.particles());
        let read: std::collections::HashSet<&str> = self.layers().flat_map(crate::links::reads).collect();
        let apart = read.into_iter().filter_map(|id| self.layer(id)).map(|l| APART * area(l));
        let scale = quality.render_scale();
        BASE + (layers.sum::<f32>() + particles.sum::<f32>() + apart.sum::<f32>()) * scale * scale
    }
}
