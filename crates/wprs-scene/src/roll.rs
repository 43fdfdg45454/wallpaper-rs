use crate::look::Playback;
use crate::resolve::Context;
use crate::runtime::Runtime;
use crate::{Content, Draw, Frame, Kind};

impl Runtime {
    fn governed(&self, i: usize) -> (f32, bool) {
        let t = &self.tapes[i];
        let layer = t.layer.as_ref().and_then(|id| self.scene.layer(id));
        let p: Option<&Playback> = layer.and_then(|l| match &l.kind {
            Kind::Video { playback, .. } | Kind::Image { playback, .. } => playback.as_ref(),
            _ => None,
        });
        let cx = self.context();
        let rate = playable(t.speed.unwrap_or_else(|| p.map_or(1.0, |p| p.rate.get(&cx))));
        (rate, t.held.unwrap_or_else(|| p.is_none_or(|p| p.playing.get(&cx))))
    }

    pub(crate) fn roll(&mut self, step: f64) {
        let sighted = self.sighted.get();
        for i in 0..self.tapes.len() {
            let (rate, playing) = self.governed(i);
            let last = self.length(&self.tapes[i].asset);
            let still = self.still;
            let t = &mut self.tapes[i];
            (t.rate, t.playing, t.shown) = (rate, playing, i >= 64 || sighted & (1 << i) != 0);
            if !playing || !t.shown || t.ended || still {
                continue;
            }
            let end = t.end.unwrap_or(f64::INFINITY).min(last);
            t.at += step * f64::from(rate);
            if t.at < end || !end.is_finite() {
                continue;
            }
            match t.repeat && end > t.start {
                true => t.at = t.start + (t.at - t.start) % (end - t.start),
                false => {
                    (t.at, t.ended) = (end, true);
                    let layer = t.layer.clone().filter(|_| self.ended.len() < crate::MAX_FIRED);
                    self.ended.extend(layer);
                }
            }
        }
    }

    pub(crate) fn sight(&self, frame: &Frame<'_>) -> bool {
        let mut seen = 0u64;
        let mut mark = |name: &str| {
            let found = self.tapes.iter().take(64).position(|t| t.asset == name);
            seen |= found.map_or(0, |i| 1 << i);
        };
        for (_, d) in frame.draws.iter().enumerate().filter(|(i, _)| needed(frame, *i, 4)) {
            if let Content::Image { asset, .. } | Content::Video { asset, .. } = &d.content {
                mark(asset);
            }
            d.mask_texture.as_deref().into_iter().for_each(&mut mark);
            for e in &frame.effects[d.effects.clone()] {
                e.mask_texture.as_deref().into_iter().for_each(&mut mark);
                e.textures.iter().for_each(|(_, name)| mark(name));
                let fixed = e.descriptor.params.iter().filter(|p| p.kind.asset());
                fixed.filter_map(|p| p.named(e.effect)).for_each(&mut mark);
            }
        }
        let fresh = seen & !self.sighted.replace(seen);
        let rolling =
            |(i, t): (usize, &crate::Tape)| fresh & (1 << i) != 0 && t.playing && !t.ended && t.rate > 0.0;
        self.tapes.iter().take(64).enumerate().any(rolling)
    }
}

pub(crate) fn playable(rate: f32) -> f32 {
    if rate > 0.0 { rate.clamp(0.1, 4.0) } else { 0.0 }
}

fn reads(frame: &Frame<'_>, d: &Draw<'_>, id: &str) -> bool {
    let effects = &frame.effects[d.effects.clone()];
    let masks = d.mask.into_iter().chain(effects.iter().filter_map(|e| e.mask));
    let fed =
        effects.iter().flat_map(|e| e.effect.params.values()).filter_map(|v| v.binding()?.layer.as_deref());
    masks.filter_map(|m| m.layer.as_deref()).chain(fed).any(|l| l == id)
}

fn needed(frame: &Frame<'_>, i: usize, depth: u32) -> bool {
    let draws = &frame.draws;
    if !draws[i].hidden {
        return true;
    }
    let mut names = draws[..=i].iter().enumerate().filter(|(k, a)| *k == i || a.end > i);
    let read = |id: &str, j: usize| j != i && reads(frame, &draws[j], id) && needed(frame, j, depth - 1);
    depth > 0 && names.any(|(_, a)| (0..draws.len()).any(|j| read(&a.layer.id, j)))
}

impl Context<'_> {
    pub(crate) fn rolls(&self, asset: &str, unknown: bool) -> bool {
        let found = self.tapes.iter().find(|t| t.asset == asset);
        found.map_or(unknown, |t| t.shown && t.playing && !t.ended && t.rate > 0.0)
    }
}
