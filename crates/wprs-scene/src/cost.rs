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

    fn zoom(&self, l: &Layer) -> f32 {
        match &l.transform.scale {
            crate::Value::Fixed(crate::Vec2([x, y])) => (x * y).abs().sqrt().max(1e-3),
            crate::Value::Bound(_) => 1.0,
        }
    }

    pub fn thin(&self, l: &Layer, quality: Quality, magnify: f32) -> f32 {
        let mesh = matches!(l.kind, Kind::Mesh(_));
        if l.effects.is_empty() || !(mesh || matches!(l.kind, Kind::Image { .. } | Kind::Video { .. })) {
            return 1.0;
        }
        let loose = if mesh && quality == Quality::Q3 { 1.5 } else { 1.0 };
        let shown = magnify * quality.render_scale() * self.zoom(l);
        (quality.density() * loose / shown.max(1e-6)).clamp(0.125, 1.0)
    }

    fn units(&self, l: &Layer, quality: Quality, magnify: f32) -> f32 {
        let thin = self.thin(l, quality, magnify);
        thin * thin * self.flat_units(l, quality)
    }

    fn flat_units(&self, l: &Layer, quality: Quality) -> f32 {
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
        let solid = match &l.kind {
            Kind::Model(_) => self.drawn_triangles(l, quality) as f32 / crate::TRIANGLE_UNIT + APART,
            _ => 0.0,
        };
        effects.sum::<f32>() + fed.sum::<f32>() + backdrop + solid + if apart { APART } else { 0.0 }
    }

    pub fn copies_cost<'a>(
        &self,
        copies: impl Iterator<Item = &'a Layer>,
        quality: Quality,
        magnify: f32,
    ) -> f32 {
        let mut stack: Vec<&Layer> = copies.collect();
        let mut units = 0.0;
        while let Some(l) = stack.pop() {
            stack.extend(l.children());
            units += if l.flat() { self.units(l, quality, magnify) } else { 0.0 };
        }
        let scale = quality.render_scale();
        units * scale * scale
    }

    pub fn magnify(&self, (w, h): (u32, u32)) -> f32 {
        let fit = match self.canvas.fit {
            wprs_core::Fit::Tile => wprs_core::Fit::Center,
            wprs_core::Fit::Span => wprs_core::Fit::Fill,
            fit => fit,
        };
        let [cw, ch] = self.canvas.size;
        let uv = wprs_core::UvRect::compute(fit, (cw, ch), (w, h));
        let [sw, sh] = [uv.scale[0] * cw as f32, uv.scale[1] * ch as f32].map(|v| v.max(1e-3));
        (w as f32 / sw * h as f32 / sh).sqrt()
    }

    pub fn cost(&self, quality: Quality) -> f32 {
        self.cost_at(quality, 1.0)
    }

    pub fn cost_at(&self, quality: Quality, magnify: f32) -> f32 {
        let areas = self.areas();
        let whole = self.canvas.camera.is_some();
        let area = |l: &Layer| areas.get(&std::ptr::from_ref(l)).copied().filter(|_| !whole).unwrap_or(1.0);
        let layers = self.layers().filter(|l| l.flat()).map(|l| self.units(l, quality, magnify) * area(l));
        let glow = crate::glow::glows(&self.canvas)[0].as_deref().map_or(0.0, |g| self.units(g, quality, magnify));
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
        BASE + (layers.sum::<f32>() + particles.sum::<f32>() + apart.sum::<f32>() + glow) * scale * scale
    }
}
