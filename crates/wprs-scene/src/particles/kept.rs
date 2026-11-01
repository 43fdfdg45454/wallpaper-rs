use super::Force;
use super::budget::MAX_LONG;
use super::{Render, Rope, System, Trail};
use crate::{Kind, Scene, Tier};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Kept {
    pub length: u32,
    pub kept: u32,
    pub stride: u32,
    pub long: bool,
}

impl Trail {
    pub fn long(&self) -> bool {
        let more = [self.segments.is_some(), self.subdivide.is_some(), self.uv_scale.is_some()];
        self.length > 16 || more.contains(&true) || self.uv_scroll.is_some() || self.textured
    }

    pub fn kept(&self) -> Kept {
        let length = self.length.clamp(1, MAX_LONG);
        if !self.long() {
            return Kept { length, kept: length.min(16), stride: 1, long: false };
        }
        let kept = self.segments.map_or(length, |g| g + 1).clamp(1, MAX_LONG);
        let stride = if kept > 1 { (length - 1).div_ceil(kept - 1).max(1) } else { 1 };
        Kept { length, kept, stride, long: true }
    }
}

impl Kept {
    pub fn capped(self, tier: Tier, cap: u32) -> Kept {
        let most = if self.long { tier.long_trail() } else { tier.trail() };
        let kept = self.kept.min(most).min(cap);
        let stride = match self.long && kept > 1 && kept < self.kept {
            true => (self.length - 1).div_ceil(kept - 1).max(self.stride),
            false => self.stride,
        };
        Kept { kept, stride, ..self }
    }
}

fn renders<'a>(scene: &'a Scene, id: &'a str, s: &'a System) -> impl Iterator<Item = &'a Render> {
    let own = scene.layers().filter_map(move |l| match &l.kind {
        Kind::Particles { system, render: Some(r), .. } if system == id => Some(&**r),
        _ => None,
    });
    std::iter::once(&s.render).chain(own)
}

pub(super) fn asked(scene: &Scene, id: &str, s: &System) -> Kept {
    let all = renders(scene, id, s).filter_map(|r| r.trail.as_ref()).map(Trail::kept);
    let long = renders(scene, id, s).filter_map(|r| r.trail.as_ref()).any(Trail::long);
    let most = all.max_by_key(|k| (k.kept, k.length)).unwrap_or_default();
    Kept { long: long || most.long, ..most }
}

pub(super) fn roped(scene: &Scene, id: &str, s: &System) -> Option<Rope> {
    renders(scene, id, s).find_map(|r| r.rope)
}

pub(super) fn flocks(s: &System) -> bool {
    s.forces.iter().any(|f| matches!(f.force, Force::Boids(_)))
}
