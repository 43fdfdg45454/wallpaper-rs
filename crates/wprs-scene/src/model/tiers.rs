use super::{MAX_TEXTURE, Model, PHONE_TEXTURE};
use crate::Tier;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ModelCaps {
    pub triangles: u64,
    pub bones: usize,
    pub texture: u32,
    pub shadows: usize,
}

pub fn caps(tier: Tier, phone: bool) -> ModelCaps {
    let triangles = [50_000, 150_000, 500_000, 2_000_000][tier as usize];
    let shadows = [0, 0, 1, 2][tier as usize];
    let texture = if phone || tier <= Tier::T1 { PHONE_TEXTURE } else { MAX_TEXTURE };
    ModelCaps { triangles, bones: crate::MAX_BONES, texture, shadows }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verdict {
    pub tier: Tier,
    pub fits: bool,
    pub reasons: Vec<String>,
}

pub fn verdict(models: &[&Model], bones: usize, tier: Tier) -> Verdict {
    let c = caps(tier, false);
    let triangles: u64 = models.iter().map(|m| m.triangles).sum();
    let side = models.iter().map(|m| m.texture_side()).max().unwrap_or(0);
    let mut reasons = Vec::new();
    if triangles > c.triangles {
        reasons.push(format!("{triangles} triangles, more than {}", c.triangles));
    }
    if bones > c.bones {
        reasons.push(format!("{bones} bones, more than {}", c.bones));
    }
    if side > c.texture {
        reasons.push(format!("textures of {side} px are drawn at {} px", c.texture));
    }
    let fits = triangles <= c.triangles && bones <= c.bones;
    Verdict { tier, fits, reasons }
}
