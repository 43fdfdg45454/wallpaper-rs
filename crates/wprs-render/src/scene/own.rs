use super::Src;
use super::item::{Fx, Rect, linear};
use super::paint::Painter;
use super::pipes::Mode;
use super::ready::UNREADY;
use super::stops::{More, STOPPED, stopped};
use wprs_scene::{Affine, Descriptor, EffectDraw, MaskMode, Output, PassBlend};

mod passes;

const AT: [usize; 9] = [0, 2, 3, 4, 5, 7, 8, 9, 10];

struct Inputs {
    previous: Src,
    current: Src,
    source: Src,
    mask: Src,
    front: Option<(usize, usize)>,
    base: usize,
    made: usize,
}

fn target(d: &Descriptor, made: usize, name: &str) -> Option<usize> {
    d.targets.keys().position(|t| t == name).map(|k| made + k)
}

pub(crate) fn mode(blend: Option<PassBlend>) -> Mode {
    match blend {
        None | Some(PassBlend::Replace) => Mode::Replace,
        Some(PassBlend::Normal) => Mode::Normal,
        Some(PassBlend::Add) => Mode::Add,
        Some(PassBlend::Multiply) => Mode::Multiply,
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn own(
        &mut self,
        i: usize,
        e: &EffectDraw<'_>,
        source: Src,
        current: Src,
        rect: Rect,
        uv: Affine,
    ) -> Src {
        let d: &Descriptor = e.descriptor;
        let stops = stopped(d.key);
        if stops && !self.refused.contains_key(&d.id) {
            self.refused.insert(d.id.clone(), STOPPED.into());
        } else if !stops && self.refused.get(&d.id).is_some_and(|why| why == STOPPED) {
            self.refused.remove(&d.id);
        }
        if let Some(why) = super::clock::slow(d.key) {
            self.refused.entry(d.id.clone()).or_insert(why);
        }
        if self.refused.contains_key(&d.id) {
            return current;
        }
        if !self.r.pipes().ready(d, e.variant, [self.format, self.state_format]) {
            self.refused.insert(d.id.clone(), UNREADY.into());
            tracing::warn!("the effect {} was left out: {UNREADY}", d.id);
            return current;
        }
        self.drawn.push(d.key);
        let fed = self.fed(i, e, rect, uv);
        let (fx, mask) = self.base(e, uv);
        let mask = fed.mask.unwrap_or(mask);
        let lut = self.lut(e);
        let (made, start) = (self.made.len(), self.ops.len());
        let side = |v: f32, s: f32| (v * s * e.scale).ceil().max(1.0) as u32;
        let sim = d.state.as_ref().map(|s| self.state(i, e, (side(rect.w, s.scale), side(rect.h, s.scale))));
        if sim.is_some_and(|s| s.is_none()) || !self.targets(d, rect, e.scale) {
            fed.all().for_each(|s| self.release(s));
            return current;
        }
        let sim = sim.flatten();
        let draw = &self.frame.draws[i];
        let c = draw.tint.0.map(|v| if self.srgb { v } else { linear(v) });
        let b = draw.brightness;
        let mut more = More {
            layer_color: [c[0] * b, c[1] * b, c[2] * b, draw.tint.0[3]],
            layer_alpha: draw.opacity,
            ..self.more
        };
        let (base, mut previous, last) = (self.outputs.len(), current, d.passes.len() - 1);
        for (n, pass) in d.passes.iter().enumerate() {
            if let Some([a, b]) = &pass.swap {
                if let (Some(a), Some(b)) = (target(d, made, a), target(d, made, b)) {
                    self.made.swap(a, b);
                }
                self.outputs.push(previous);
                continue;
            }
            let state = pass.output == Output::State;
            let runs = sim.filter(|_| state).map_or(1, |(.., ticks)| ticks);
            for k in 0..runs {
                let front = sim.map(|(id, ..)| (id, self.front(id)));
                let mut tex = [Src::Clear; 11];
                (tex[1], tex[6]) = (mask, lut);
                let names = pass.copy.iter().chain(&pass.inputs).take(AT.len());
                let at = Inputs { previous, current, source, mask, front, base, made };
                for (k, name) in AT.iter().zip(names) {
                    tex[*k] = self.input(e, &fed, &at, name).unwrap_or(Src::Clear);
                }
                let written = pass.target.as_deref().and_then(|t| target(d, made, t)).map(|k| self.made[k]);
                let out = front.filter(|_| state).map(|(id, side)| Src::State(id, 1 - side)).or(written);
                let scale = if n == last || written.is_some() { pass.scale } else { pass.scale * e.scale };
                let fx = Fx { info: [fx.info[0], k as f32, n as f32, 0.0], ..fx };
                let Some((target, slot)) = self.place(rect, out, scale.clamp(0.05, 1.0), tex[0], fx) else {
                    break;
                };
                let load = written.is_some_and(|w| !self.unclear(w, start));
                previous = self.record(d, (n, pass), target, slot, tex, (&mut more, e.variant, load));
                if let Some((id, _)) = front.filter(|_| state) {
                    self.flip(id);
                }
            }
            self.outputs.push(previous);
        }
        if d.mask_mode == MaskMode::Mix && e.mask.is_some() {
            let mut tex = super::effects::EMPTY;
            (tex[0], tex[1], tex[2]) = (previous, mask, current);
            previous = self.bare("fs_mask_mix", rect, tex, fx);
        }
        let kept = [previous, current, source];
        for k in base..self.outputs.len() {
            let s = self.outputs[k];
            if !kept.contains(&s) {
                self.release(s);
            }
        }
        for k in made..self.made.len() {
            let s = self.made[k];
            if !kept.contains(&s) {
                self.release(s);
            }
        }
        self.outputs.truncate(base);
        self.made.truncate(made);
        fed.all().filter(|s| !kept.contains(s)).for_each(|s| self.release(s));
        previous
    }
}
