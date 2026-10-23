use super::{MAX_CURVE, is_zero};
use crate::Color;
use crate::validate::Check;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Frames {
    Time,
    Life,
    Random,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub texture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<Frames>,
    pub fps: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mix: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-curve" = true))]
    pub size_over_life: Vec<[f32; 2]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub color_over_life: Vec<(f32, Color)>,
    #[serde(skip_serializing_if = "is_zero")]
    pub stretch: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail: Option<Trail>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trail {
    pub length: u32,
    pub width: f32,
}

impl Default for Render {
    fn default() -> Self {
        let (texture, frames, trail, size_over_life, color_over_life) = Default::default();
        Render {
            texture,
            frames,
            fps: 12.0,
            mix: false,
            size_over_life,
            color_over_life,
            stretch: 0.0,
            trail,
        }
    }
}

impl Render {
    pub fn luts(&self) -> ([f32; MAX_CURVE], [[f32; 4]; MAX_CURVE]) {
        let at = |i: usize| i as f32 / (MAX_CURVE - 1) as f32;
        let sizes: Vec<(f32, [f32; 4])> = self.size_over_life.iter().map(|k| (k[0], [k[1]; 4])).collect();
        let colors: Vec<(f32, [f32; 4])> = self.color_over_life.iter().map(|(t, c)| (*t, c.0)).collect();
        (std::array::from_fn(|i| curve(&sizes, at(i))[0]), std::array::from_fn(|i| curve(&colors, at(i))))
    }
}

fn curve(keys: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
    let n = keys.partition_point(|k| k.0 <= t);
    match (keys.get(n.wrapping_sub(1)), keys.get(n)) {
        (None, None) => [1.0; 4],
        (Some(a), None) | (None, Some(a)) => a.1,
        (Some(a), Some(b)) => {
            let f = (t - a.0) / (b.0 - a.0).max(1e-6);
            std::array::from_fn(|k| a.1[k] + (b.1[k] - a.1[k]) * f)
        }
    }
}

impl Check<'_> {
    pub(super) fn render(&mut self, p: &str, r: &Render) {
        if let Some(id) = &r.texture {
            self.known(&format!("{p}/render/texture"), "asset", id, self.scene.assets.contains_key(id));
        }
        self.range(&format!("{p}/render/fps"), r.fps, 0.0, 240.0);
        self.range(&format!("{p}/render/stretch"), r.stretch, 0.0, 10.0);
        let sizes = r.size_over_life.iter().map(|k| k[0]);
        let colors = r.color_over_life.iter().map(|k| k.0);
        for (name, times) in
            [("size_over_life", sizes.collect::<Vec<_>>()), ("color_over_life", colors.collect())]
        {
            let q = format!("{p}/render/{name}");
            self.count(&q, times.len(), MAX_CURVE);
            self.keys_sorted(&q, times.iter().copied());
            times.iter().for_each(|t| self.range(&q, *t, 0.0, 1.0));
        }
        r.size_over_life
            .iter()
            .for_each(|k| self.range(&format!("{p}/render/size_over_life"), k[1], 0.0, 100.0));
        for (k, (_, c)) in r.color_over_life.iter().enumerate() {
            self.color(&format!("{p}/render/color_over_life/{k}/1"), *c);
        }
        if let Some(t) = &r.trail {
            self.range(&format!("{p}/render/trail/length"), t.length as f32, 1.0, 16.0);
            self.range(&format!("{p}/render/trail/width"), t.width, 0.0, 1.0);
        }
    }
}
