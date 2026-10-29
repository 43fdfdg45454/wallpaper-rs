use crate::{Kind, Layer, Quality, Scene, Shape, Value};

const BACKDROP: f32 = 2.0;
const APART: f32 = 1.0;
const BASE: f32 = 1.5;

fn shape(s: &Shape) -> [f32; 2] {
    match *s {
        Shape::Rect { size, .. } | Shape::Ellipse { size } | Shape::Ring { size, .. } => size,
        Shape::Polygon { size, .. } | Shape::Star { size, .. } => [size; 2],
        Shape::Line { to, width } => [to[0].abs() + width, to[1].abs() + width],
    }
}

fn scale(layer: &Layer) -> f32 {
    match &layer.transform.scale {
        Value::Fixed(crate::Vec2([x, y])) => (x * y).abs(),
        Value::Bound(_) => 1.0,
    }
}

impl Scene {
    pub fn area(&self, layer: &Layer) -> f32 {
        let canvas = self.canvas.size.map(|v| v as f32);
        let size = match &layer.kind {
            Kind::Group { layers, .. } => return layers.iter().map(|l| self.area(l)).sum::<f32>().min(1.0),
            Kind::Image { size, .. } | Kind::Video { size, .. } | Kind::Solid { size, .. } => {
                size.unwrap_or(canvas)
            }
            Kind::Adjustment { size, .. } => size.unwrap_or(canvas),
            Kind::Shape { shape: s, .. } => shape(s),
            Kind::Spectrum(spectrum) => spectrum.size,
            Kind::Text(text) => {
                let chars = match &text.text {
                    Value::Fixed(t) => t.lines().map(|l| l.chars().count()).max().unwrap_or(0).max(1),
                    Value::Bound(_) => 8,
                };
                let lines = match &text.text {
                    Value::Fixed(t) => t.lines().count().max(1),
                    Value::Bound(_) => 1,
                };
                [text.size * 0.6 * chars as f32, text.size * 1.2 * lines as f32]
            }
            _ => canvas,
        };
        (size[0] * size[1] * scale(layer) / (canvas[0] * canvas[1]).max(1.0)).clamp(0.0, 1.0)
    }

    pub fn cost(&self, quality: Quality) -> f32 {
        let layers = self.layers().filter(|l| l.flat()).map(|l| {
            let area = self.area(l);
            let effects = l.effects.iter().filter_map(|e| {
                let d = self.effect(&e.effect)?;
                let scale = quality.effect_scale(d.class(), e.optional)? * d.scale_at(quality);
                let scale = if d.passes.len() > 1 { scale } else { 1.0 };
                Some(d.units() * scale * scale)
            });
            let apart = !l.effects.is_empty() || l.mask.is_some() || !l.blend.fixed() || l.cache;
            let backdrop = if l.blend.fixed() { 0.0 } else { BACKDROP };
            (effects.sum::<f32>() + backdrop + if apart { APART } else { 0.0 }) * area
        });
        let particles = self.layers().filter_map(|l| match &l.kind {
            Kind::Particles { system, .. } => self.particles.get(system),
            _ => None,
        });
        let canvas = self.canvas.size;
        let particles =
            particles.map(|s| (s.capacity as f32 / 100_000.0 + s.overdraw(canvas)) * quality.particles());
        let scale = quality.render_scale();
        BASE + (layers.sum::<f32>() + particles.sum::<f32>()) * scale * scale
    }
}
