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

pub fn fur_shells(layers: u32, quality: crate::Quality) -> u32 {
    let part = [0.25, 0.5, 0.75, 1.0][quality as usize];
    ((layers.clamp(1, 32) as f32 * part).ceil() as u32).max(1)
}

pub fn shadow_side(
    beam: crate::Beam,
    set: Option<u32>,
    (tier, phone): (Tier, bool),
    q: crate::Quality,
) -> u32 {
    let base = match beam {
        crate::Beam::Directional => 2048,
        crate::Beam::Spot => 1024,
        _ => 512,
    };
    let cap = match tier {
        Tier::T0 | Tier::T1 => return 0,
        _ if phone => 1024,
        Tier::T2 => 2048,
        Tier::T3 => 4096,
    };
    let side = set.unwrap_or(base).clamp(256, cap);
    if q == crate::Quality::Q0 { (side / 2).max(256) } else { side }
}
