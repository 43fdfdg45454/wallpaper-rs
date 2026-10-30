use super::item::{Fx, Rect, channel, rows, scale_of};
use super::paint::Painter;
use super::pipes::{Mode, Shader};
use super::{Fill, Op, Src};
use wprs_scene::{Affine, CHAIN_ENTRY, Descriptor, EffectDraw, MaskMode, Output, ParamKind, descriptor};

pub(crate) const EMPTY: [Src; 7] =
    [Src::Clear, Src::White, Src::Clear, Src::Clear, Src::Clear, Src::Clear, Src::Void];

impl Painter<'_, '_, '_> {
    fn pass(
        &mut self,
        shader: Shader,
        rect: Rect,
        out: Option<Src>,
        scale: f32,
        tex: [Src; 7],
        fx: Fx,
    ) -> Src {
        let Some((target, slot)) = self.place(rect, out, scale, tex[0], fx) else { return tex[0] };
        let mut all = [Src::Clear; 11];
        all[..7].copy_from_slice(&tex);
        let (more, mode, load, vertices, tex) = (None, Mode::Replace, false, 3, self.set(all));
        self.ops.push(Op::Effect(Fill { target, shader, slot, tex, more, mode, load, vertices }));
        target
    }

    pub(crate) fn set(&mut self, tex: [Src; 11]) -> u32 {
        self.sets.push(tex);
        self.sets.len() as u32 - 1
    }

    pub(crate) fn bare(&mut self, entry: &'static str, rect: Rect, tex: [Src; 7], fx: Fx) -> Src {
        let lease = Some(&mut *self.lease);
        let plain = self.r.pipes().chain(&self.r.device, std::iter::empty::<&Descriptor>(), lease);
        self.pass(Shader::Fx(plain, entry), rect, None, 1.0, tex, fx)
    }

    pub(crate) fn place(
        &mut self,
        rect: Rect,
        out: Option<Src>,
        scale: f32,
        first: Src,
        fx: Fx,
    ) -> Option<(Src, u32)> {
        let size = ((rect.w * scale).ceil().max(1.0) as u32, (rect.h * scale).ceil().max(1.0) as u32);
        let out = out.or_else(|| self.acquire(size))?;
        let (w, h) = self.size_of(first);
        let px = scale_of(self.screen) * w as f32 / rect.w.max(1.0);
        let f = self.full;
        let (x0, y0) = (((f.x - rect.x) / rect.w).max(0.0), ((f.y - rect.y) / rect.h).max(0.0));
        let (x1, y1) = (((f.x + f.w - rect.x) / rect.w).min(1.0), ((f.y + f.h - rect.y) / rect.h).min(1.0));
        let fx = Fx {
            texel: [1.0 / w as f32, 1.0 / h as f32, w as f32, h as f32],
            info: [fx.info[0], fx.info[1], fx.info[2], px],
            area: [rect.x, rect.y, rect.w, rect.h],
            clip: [x0, y0, x1.max(x0), y1.max(y0)],
            ..fx
        };
        Some((out, self.pool.push(&fx)))
    }

    pub(crate) fn base(&mut self, e: &EffectDraw<'_>, layer_uv: Affine) -> (Fx, Src) {
        let images = self.images;
        let mask = e.mask_texture.as_deref().and_then(|id| images.get(id));
        let mask = mask.map_or(Src::White, |image| self.ext(&image.0));
        let (mut mask_m0, mask_m1) = rows(layer_uv);
        mask_m0[3] = 1.0 - e.mask_strength;
        let fx = Fx {
            slots: e.slots,
            texel: [0.0; 4],
            channel: e.mask.map_or([0.0, 0.0, 0.0, 1.0], |m| channel(m.channel, m.texture.is_some())),
            mask_m0,
            mask_m1,
            info: [f32::from(u8::from(e.mask.is_some_and(|m| m.invert))), 0.0, 0.0, 0.0],
            area: [0.0; 4],
            clip: [0.0, 0.0, 1.0, 1.0],
            velocity: [e.velocity[0], e.velocity[1], 0.0, 0.0],
        };
        (fx, mask)
    }

    pub(crate) fn effects(&mut self, i: usize, input: Src, rect: Rect, layer_uv: Affine) -> Src {
        let frame = self.frame;
        let list = &frame.effects[frame.draws[i].effects.clone()];
        let (mut current, mut k) = (input, 0);
        while k < list.len() {
            let n = list[k].chain.clamp(1, list.len() - k);
            let n = if list[k..k + n].iter().any(|e| self.cut(i, e)) { 1 } else { n };
            if self.cut(i, &list[k]) {
                k += 1;
                continue;
            }
            let next = self.step(i, &list[k..k + n], (input, current), rect, layer_uv);
            if current != input && current != next {
                self.release(current);
            }
            (current, k) = (next, k + n);
        }
        current
    }

    pub(crate) fn chain(&mut self, list: &[EffectDraw<'_>], input: Src, rect: Rect, layer_uv: Affine) -> Src {
        let key =
            self.r.pipes().chain(&self.r.device, list.iter().map(|e| e.descriptor), Some(&mut *self.lease));
        let (mut fx, _) = self.base(&list[0], layer_uv);
        let mut base = 0;
        for e in list {
            let n = e.descriptor.params.len();
            fx.slots[base..base + n].copy_from_slice(&e.slots[..n]);
            base += n;
        }
        let mut tex = EMPTY;
        tex[0] = input;
        self.pass(Shader::Fx(key, CHAIN_ENTRY), rect, None, 1.0, tex, fx)
    }

    pub(crate) fn effect(
        &mut self,
        i: usize,
        e: &EffectDraw<'_>,
        source: Src,
        current: Src,
        rect: Rect,
        layer_uv: Affine,
    ) -> Src {
        let Some(d) = descriptor(&e.descriptor.id) else { return current };
        let key = self.r.pipes().chain(&self.r.device, std::iter::once(d), Some(&mut *self.lease));
        let fed = self.fed(i, e, rect, layer_uv);
        let (fx, mask) = self.base(e, layer_uv);
        let mask = fed.mask.unwrap_or(mask);
        let lut = self.lut(e);
        let tune = d.scale_at(self.frame.quality);
        let sim = match d.state {
            Some(s) => {
                let side = |v: f32| (v * s.scale * tune).ceil().max(1.0) as u32;
                let Some(sim) = self.state(i, e, (side(rect.w), side(rect.h))) else {
                    fed.all().for_each(|s| self.release(s));
                    return current;
                };
                Some(sim)
            }
            None => None,
        };
        let (base, mut previous, last) = (self.outputs.len(), current, d.passes.len() - 1);
        for (n, pass) in d.passes.iter().enumerate() {
            let state = pass.output == Output::State;
            let runs = sim.filter(|_| state).map_or(1, |(.., ticks)| ticks);
            for k in 0..runs {
                let front = sim.map(|(id, ..)| (id, self.front(id)));
                let mut tex = [Src::Clear, mask, Src::Clear, Src::Clear, Src::Clear, Src::Clear, lut];
                for (at, name) in [0, 2, 3, 4, 5].into_iter().zip(&pass.inputs) {
                    tex[at] = match name.as_str() {
                        "previous" => previous,
                        "input" => current,
                        "source" => source,
                        "mask" => mask,
                        "backdrop" => self.backdrop.unwrap_or(Src::Clear),
                        "state" => front.map_or(Src::Clear, |(id, side)| Src::State(id, side)),
                        other => match other.split_once(':') {
                            Some(("pass", k)) => {
                                k.parse().ok().and_then(|k: usize| self.outputs[base..].get(k).copied())
                            }
                            Some(("asset", p)) => fed.get(p).or_else(|| self.asset(e, p, ParamKind::Texture)),
                            _ => None,
                        }
                        .unwrap_or(Src::Clear),
                    };
                }
                let out = front.filter(|_| state).map(|(id, side)| Src::State(id, 1 - side));
                let scale = if n == last { pass.scale } else { pass.scale * e.scale };
                let info = [fx.info[0], k as f32, n as f32, 0.0];
                let fx = Fx { info, ..fx };
                previous =
                    self.pass(Shader::Fx(key, &pass.entry), rect, out, scale.clamp(0.05, 1.0), tex, fx);
                if let Some((id, _)) = front.filter(|_| out.is_some()) {
                    self.flip(id);
                }
            }
            self.outputs.push(previous);
        }
        if d.mask_mode == MaskMode::Mix && e.mask.is_some() {
            let mut tex = EMPTY;
            (tex[0], tex[1], tex[2]) = (previous, mask, current);
            previous = self.bare("fs_mask_mix", rect, tex, fx);
        }
        while self.outputs.len() > base {
            let out = self.outputs.pop().expect("longer than base");
            if ![previous, current, source].contains(&out) {
                self.release(out);
            }
        }
        fed.all().filter(|s| *s != previous).for_each(|s| self.release(s));
        previous
    }
}
