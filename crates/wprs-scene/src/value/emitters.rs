use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};

pub const MAX_EFFECT_EMITTERS: usize = 64;
pub const EMITTER_TEXELS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectEmitter {
    pub from: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<[f32; 2]>,
    #[serde(default = "up")]
    #[schemars(extend("x-range" = [-360, 360]))]
    pub angle: f32,
    #[schemars(extend("x-range" = [0, 64]))]
    pub strength: f32,
}

fn up() -> f32 {
    -90.0
}

fn word(v: f32, lo: f32, hi: f32) -> [u8; 2] {
    let w = (((v - lo) / (hi - lo)).clamp(0.0, 1.0) * 65535.0).round() as u16;
    w.to_be_bytes()
}

impl EffectEmitter {
    pub fn fits(&self) -> bool {
        let place = |p: [f32; 2]| p.iter().all(|v| v.is_finite() && (-2.0..=4.0).contains(v));
        place(self.from)
            && self.to.is_none_or(place)
            && (-360.0..=360.0).contains(&self.angle)
            && (0.0..=64.0).contains(&self.strength)
    }

    fn texels(&self) -> [[u8; 4]; EMITTER_TEXELS] {
        let pair = |[x, y]: [f32; 2]| {
            let ([a, b], [c, d]) = (word(x, -2.0, 4.0), word(y, -2.0, 4.0));
            [a, b, c, d]
        };
        let ([a, b], [c, d]) = (word(self.angle, -360.0, 360.0), word(self.strength, 0.0, 64.0));
        [pair(self.from), pair(self.to.unwrap_or(self.from)), [a, b, c, d]]
    }
}

pub fn emitter_texels(list: &[EffectEmitter]) -> Vec<[u8; 4]> {
    list.iter().take(MAX_EFFECT_EMITTERS).flat_map(EffectEmitter::texels).collect()
}

pub fn emitter_key(list: &[EffectEmitter]) -> String {
    let mut h = DefaultHasher::new();
    emitter_texels(list).hash(&mut h);
    format!("emitters:{:016x}", h.finish())
}
