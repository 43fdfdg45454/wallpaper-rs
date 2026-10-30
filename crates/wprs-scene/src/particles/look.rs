use super::MAX_CURVE;
use crate::validate::Check;
use crate::{Color, Value};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Frames {
    Time,
    Life,
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stack {
    Newest,
    Oldest,
    Largest,
    Smallest,
    Lowest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub texture: Option<Value<String>>,
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
    #[serde(skip_serializing_if = "Stretch::is_off")]
    pub stretch: Stretch,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail: Option<Trail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 16]))]
    pub brightness: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cutout: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [1, 64]))]
    pub cycles: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Stack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope: Option<Rope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orient: Option<Orient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°"))]
    pub angle: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub normal: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refract: Option<Refract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "material"))]
    pub shader: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trail {
    pub length: u32,
    pub width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 7]))]
    pub subdivide: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_scroll: Option<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub textured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(from = "Stretching", into = "Stretching")]
pub struct Stretch {
    pub time: f32,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum Stretching {
    Time(f32),
    Range(Span),
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Span {
    #[serde(default)]
    time: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Rope {
    #[schemars(extend("x-range" = [0, 7]))]
    pub subdivide: u32,
    #[schemars(extend("x-unit" = "px"))]
    pub width: f32,
    pub uv_scale: f32,
    pub uv_scroll: f32,
    pub smooth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Orient {
    Screen,
    Velocity,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Refract {
    #[schemars(extend("x-ref" = "image"))]
    pub normal: String,
    #[schemars(extend("x-range" = [0, 1]))]
    pub amount: f32,
}

impl From<Stretching> for Stretch {
    fn from(s: Stretching) -> Self {
        match s {
            Stretching::Time(time) => Stretch { time, min: None, max: None },
            Stretching::Range(Span { time, min, max }) => Stretch { time, min, max },
        }
    }
}

impl From<Stretch> for Stretching {
    fn from(s: Stretch) -> Self {
        match (s.min, s.max) {
            (None, None) => Stretching::Time(s.time),
            (min, max) => Stretching::Range(Span { time: s.time, min, max }),
        }
    }
}

impl Stretch {
    fn is_off(&self) -> bool {
        *self == Stretch::default()
    }
}

impl Default for Rope {
    fn default() -> Self {
        Rope { subdivide: 2, width: 4.0, uv_scale: 1.0, uv_scroll: 0.0, smooth: true }
    }
}

impl Default for Render {
    fn default() -> Self {
        let (texture, frames, trail, size_over_life, color_over_life) = Default::default();
        let (brightness, cutout, cycles, rope, orient, angle, normal, refract, shader) = Default::default();
        Render {
            texture,
            frames,
            fps: 12.0,
            mix: false,
            size_over_life,
            color_over_life,
            stretch: Stretch::default(),
            trail,
            brightness,
            cutout,
            cycles,
            order: None,
            rope,
            orient,
            angle,
            normal,
            lit: false,
            refract,
            shader,
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
    pub(crate) fn render(&mut self, p: &str, r: &Render) {
        if let Some(texture) = &r.texture {
            self.named(&format!("{p}/render/texture"), texture);
        }
        self.range(&format!("{p}/render/fps"), r.fps, 0.0, 240.0);
        self.range(&format!("{p}/render/stretch"), r.stretch.time, 0.0, 10.0);
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
            self.range(&format!("{p}/render/trail/length"), t.length as f32, 1.0, 64.0);
            self.range(&format!("{p}/render/trail/width"), t.width, 0.0, 1.0);
        }
        self.render2(&format!("{p}/render"), r);
    }
}
