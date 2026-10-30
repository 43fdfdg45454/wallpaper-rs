use crate::{Affine, Kind, Layer, Param, Scene, Value, Vec2};
use geometry::{fixed, quad, shape, union};

mod geometry;

pub type Bounds = Option<[f32; 4]>;

pub fn covered(bounds: Bounds, [x, y, w, h]: [f32; 4]) -> f32 {
    let Some([x0, y0, x1, y1]) = bounds.filter(|b| b.iter().all(|v| v.is_finite())) else { return 1.0 };
    let side = |lo: f32, hi: f32, start: f32, length: f32| (hi.min(start + length) - lo.max(start)).max(0.0);
    (side(x0, x1, x, w) * side(y0, y1, y, h) / (w * h).max(1.0)).clamp(0.0, 1.0)
}

impl Scene {
    pub fn bounds(&self) -> Vec<(&Layer, Bounds)> {
        let mut out = Vec::new();
        self.walk(&self.layers, Some(Affine::IDENTITY), &mut out);
        out
    }

    fn walk<'a>(
        &'a self,
        layers: &'a [Layer],
        outer: Option<Affine>,
        out: &mut Vec<(&'a Layer, Bounds)>,
    ) -> Bounds {
        let mut all = Some([f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
        for layer in layers.iter().filter(|l| l.flat()) {
            let world = outer.zip(self.local(layer)).map(|(o, l)| o.then(l));
            let at = out.len();
            out.push((layer, None));
            let drawn = match &layer.kind {
                Kind::Group { layers, .. } => self.walk(layers, world, out),
                _ => self.size(layer).zip(world).map(|(size, m)| quad(layer, size, m)),
            };
            let grown =
                drawn.zip(self.reach(layer)).map(|([x0, y0, x1, y1], r)| [x0 - r, y0 - r, x1 + r, y1 + r]);
            out[at].1 = grown;
            all = union(all, grown);
        }
        all
    }

    fn local(&self, layer: &Layer) -> Option<Affine> {
        let t = &layer.transform;
        let moved = !self.scripts.is_empty() || layer.parent.is_some() || layer.attach.is_some();
        if moved || layer.anchor.is_some() || !layer.parallax.is_zero() || t.tilt.is_some() {
            return None;
        }
        let free = matches!(layer.kind, Kind::Group { .. } | Kind::Adjustment { .. });
        let center = if free { [0.0; 2] } else { self.canvas.size.map(|v| v as f32 / 2.0) };
        let Vec2(position) = t.position.as_ref().map_or(Some(Vec2(center)), fixed)?;
        let (Vec2(skew), Vec2(scale)) = (fixed(&t.skew)?, fixed(&t.scale)?);
        let m = Affine::translate(position).then(Affine::rotate(fixed(&t.rotation)?));
        Some(m.then(Affine::skew(skew)).then(Affine::scale(scale)))
    }

    fn size(&self, layer: &Layer) -> Option<[f32; 2]> {
        let canvas = self.canvas.size.map(|v| v as f32);
        match &layer.kind {
            Kind::Image { size, .. } | Kind::Video { size, .. } => *size,
            Kind::Solid { size, .. } => Some(size.unwrap_or(canvas)),
            Kind::Shape { shape: s, stroke, .. } => {
                let margin = stroke.as_ref().map_or(0.0, |s| s.width.abs());
                Some(shape(s).map(|v| v + margin))
            }
            Kind::Spectrum(spectrum) => Some(spectrum.size),
            _ => None,
        }
    }

    fn reach(&self, layer: &Layer) -> Option<f32> {
        let reach = |e: &crate::EffectUse| {
            let d = self.effect(&e.effect)?;
            let Some(id) = &d.reach else { return Some(0.0) };
            let given =
                e.params.get(id).map_or(d.param(id).map(|i| d.params[i].default.clone()), |v| match v {
                    Value::Fixed(p) => Some(p.clone()),
                    Value::Bound(_) => None,
                });
            match given {
                Some(Param::Number(n)) => Some(n.abs()),
                _ => None,
            }
        };
        layer.effects.iter().map(reach).sum()
    }
}
