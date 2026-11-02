use crate::{Kind, Layer, Model, Quality, Scene, Value, fur_shells};

pub const TRIANGLE_UNIT: f32 = 60_000.0;

fn layers(m: &crate::Material) -> u32 {
    match m.params.get("layers") {
        Some(Value::Fixed(crate::Param::Number(n))) => n.clamp(1.0, 32.0) as u32,
        _ => crate::FUR_LAYERS as u32,
    }
}

fn of(model: &Model, material: usize) -> u64 {
    let mine = model.primitives.iter().filter(|p| p.material.map(usize::from) == Some(material));
    mine.map(|p| u64::from(p.count / 3)).sum()
}

impl Scene {
    pub fn drawn_triangles(&self, l: &Layer, q: Quality) -> u64 {
        let Kind::Model(m) = &l.kind else { return 0 };
        let Some(model) = self.solids.get(&m.model) else { return 0 };
        let fur = m.materials.iter().filter(|(_, x)| x.shader.as_deref() == Some("fur"));
        let more = fur.map(|(name, x)| {
            u64::from(fur_shells(layers(x), q)) * model.material(name).map_or(0, |i| of(model, i))
        });
        model.triangles + more.sum::<u64>()
    }

    pub fn model_triangles(&self, q: Quality) -> u64 {
        if self.solids.is_empty() {
            return 0;
        }
        self.summed(&self.layers, q)
    }

    fn summed(&self, list: &[Layer], q: Quality) -> u64 {
        let shown = list.iter().filter(|l| l.visible != Value::Fixed(false));
        shown
            .map(|l| match &l.kind {
                Kind::Group { layers, .. } => self.summed(layers, q),
                _ => self.drawn_triangles(l, q),
            })
            .sum()
    }
}
