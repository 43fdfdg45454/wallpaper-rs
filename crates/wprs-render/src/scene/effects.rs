use super::Src;
use super::fed::Fed;
use super::item::{Fx, Rect};
use super::paint::Painter;
use super::pipes::Shader;
use wprs_scene::{Affine, CHAIN_ENTRY, EffectDraw, MaskMode, Output, ParamKind, PassDef, descriptor};

pub(crate) const EMPTY: [Src; 7] =
    [Src::Clear, Src::White, Src::Clear, Src::Clear, Src::Clear, Src::Clear, Src::Void];

struct At<'f, 'e> {
    key: u64,
    fx: Fx,
    mask: Src,
    lut: Src,
    rect: Rect,
    source: Src,
    current: Src,
    base: usize,
    sim: Option<usize>,
    fed: &'f Fed<'e>,
    e: &'f EffectDraw<'e>,
    last: usize,
    scale: f32,
}

impl Painter<'_, '_, '_> {
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
        let mut ticks = 0;
        let sim = match &d.state {
            Some(s) => {
                let side = |v: f32| (v * s.scale * tune).ceil().max(1.0) as u32;
                let Some((sim, _, n)) = self.state(i, e, (side(rect.w), side(rect.h))) else {
                    fed.all().for_each(|s| self.release(s));
                    return current;
                };
                ticks = if e.frozen { 0 } else { n };
                Some(sim)
            }
            None => None,
        };
        let base = self.outputs.len();
        let (last, scale) = (d.passes.len() - 1, e.scale);
        let at = At { key, fx, mask, lut, rect, source, current, base, sim, fed: &fed, e, last, scale };
        let state = |p: &PassDef| p.output == Output::State;
        let step = sim.and_then(|_| d.passes.iter().rposition(state)).map_or(0, |s| s + 1);
        let mut previous = current;
        for k in 0..ticks {
            for (n, pass) in d.passes[..step].iter().enumerate() {
                previous = self.step_pass(&at, n, pass, k, previous);
                match self.outputs.get_mut(base + n) {
                    Some(slot) => {
                        let old = std::mem::replace(slot, previous);
                        if ![current, source].contains(&old) && !self.outputs.contains(&old) {
                            self.release(old);
                        }
                    }
                    None => self.outputs.push(previous),
                }
            }
        }
        self.outputs.resize(base + step, current);
        for (n, pass) in d.passes.iter().enumerate().skip(step) {
            previous = self.step_pass(&at, n, pass, 0, previous);
            self.outputs.push(previous);
        }
        if d.mask_mode == MaskMode::Mix && e.mask.is_some() {
            let mut tex = EMPTY;
            (tex[0], tex[1], tex[2]) = (previous, mask, current);
            previous = self.bare("fs_mask_mix", rect, tex, fx);
        }
        while self.outputs.len() > base {
            let out = self.outputs.pop().expect("longer than base");
            if ![previous, current, source].contains(&out) && !self.outputs.contains(&out) {
                self.release(out);
            }
        }
        fed.all().filter(|s| *s != previous).for_each(|s| self.release(s));
        previous
    }

    fn step_pass(&mut self, at: &At<'_, '_>, n: usize, pass: &'static PassDef, k: u32, previous: Src) -> Src {
        let front = at.sim.map(|id| (id, self.front(id)));
        let mut tex = [Src::Clear, at.mask, Src::Clear, Src::Clear, Src::Clear, Src::Clear, at.lut];
        for (slot, name) in [0, 2, 3, 4, 5].into_iter().zip(&pass.inputs) {
            tex[slot] = match name.as_str() {
                "previous" => previous,
                "input" => at.current,
                "source" => at.source,
                "mask" => at.mask,
                "backdrop" => self.backdrop.unwrap_or(Src::Clear),
                "state" => front.map_or(Src::Clear, |(id, side)| Src::State(id, side)),
                other => match other.split_once(':') {
                    Some(("pass", p)) => {
                        p.parse().ok().and_then(|p: usize| self.outputs[at.base..].get(p).copied())
                    }
                    Some(("asset", p)) => at.fed.get(p).or_else(|| self.asset(at.e, p, ParamKind::Texture)),
                    _ => None,
                }
                .unwrap_or(Src::Clear),
            };
        }
        let state = pass.output == Output::State;
        let scale = if n == at.last { pass.scale } else { pass.scale * at.scale };
        let side = |v: f32| (v * scale.clamp(0.05, 1.0)).ceil().max(1.0) as u32;
        let temp = (pass.output == Output::Step)
            .then(|| self.borrow((side(at.rect.w), side(at.rect.h)), self.state_format));
        let out = front.filter(|_| state).map(|(id, side)| Src::State(id, 1 - side));
        let out = out.or(temp.flatten().map(Src::Pool));
        let fx = Fx { info: [at.fx.info[0], k as f32, n as f32, 0.0], ..at.fx };
        let done = self.pass(Shader::Fx(at.key, &pass.entry), at.rect, out, scale.clamp(0.05, 1.0), tex, fx);
        if let Some((id, _)) = front.filter(|_| out.is_some()) {
            self.flip(id);
        }
        done
    }
}
