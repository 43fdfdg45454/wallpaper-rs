use super::fed::Fed;
use super::item::{Fx, Rect, linear};
use super::paint::Painter;
use super::pipes::{Mode, Shader};
use super::ready::UNREADY;
use super::stops::{More, STOPPED, stopped};
use super::{Fill, Op, Src};
use wgpu::TextureFormat as Format;
use wprs_scene::{Affine, Descriptor, EffectDraw, MaskMode, Output, PassBlend, PassDef, Texel};

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
        let made = self.made.len();
        let side = |v: f32, s: f32| (v * s * e.scale).ceil().max(1.0) as u32;
        let sim = d.state.map(|s| self.state(i, e, (side(rect.w, s.scale), side(rect.h, s.scale))));
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
                previous =
                    self.record(d, (n, pass), target, slot, tex, (&mut more, e.variant, written.is_some()));
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

    fn input(&mut self, e: &EffectDraw<'_>, fed: &Fed<'_>, at: &Inputs, name: &str) -> Option<Src> {
        let d = e.descriptor;
        match name {
            "previous" => Some(at.previous),
            "input" => Some(at.current),
            "source" => Some(at.source),
            "mask" => Some(at.mask),
            "backdrop" => self.backdrop,
            "state" => at.front.map(|(id, side)| Src::State(id, side)),
            other => match other.split_once(':')? {
                ("pass", p) => p.parse().ok().and_then(|p: usize| self.outputs[at.base..].get(p).copied()),
                ("target", t) => target(d, at.made, t).map(|k| self.made[k]),
                ("asset", p) => fed.get(p).or_else(|| {
                    let kind = d.param(p).map(|j| d.params[j].kind)?;
                    self.asset(e, p, kind)
                }),
                _ => None,
            },
        }
    }

    fn record(
        &mut self,
        d: &Descriptor,
        (n, pass): (usize, &PassDef),
        target: Src,
        slot: u32,
        tex: [Src; 11],
        (more, variant, load): (&mut More, u32, bool),
    ) -> Src {
        let mode = mode(pass.blend);
        let plain =
            self.r.pipes().chain(&self.r.device, std::iter::empty::<&Descriptor>(), Some(&mut *self.lease));
        let (shader, grid) = match pass.copy {
            Some(_) => (Shader::Fx(plain, "fs_copy"), None),
            None => (Shader::Own(d.key, n as u8, variant), pass.grid),
        };
        for (k, at) in AT[1..].iter().enumerate() {
            let (w, h) = self.size_of(tex[*at]);
            more.sizes[k / 2][(k % 2) * 2..][..2].copy_from_slice(&[w as f32, h as f32]);
        }
        more.grid = grid.map_or([1.0; 2], |g| g.map(|v| v as f32));
        let vertices = grid.map_or(3, |[x, y]| x * y * 6);
        let more = pass.copy.is_none().then(|| self.pool.push(&*more));
        let tex = self.set(tex);
        self.ops.push(Op::Effect(Fill { target, shader, slot, tex, more, mode, load, vertices }));
        target
    }

    fn targets(&mut self, d: &Descriptor, rect: Rect, scale: f32) -> bool {
        let (max, made) = (self.r.device.limits().max_texture_dimension_2d as f32, self.made.len());
        for t in d.targets.values() {
            let format = if t.format == Texel::Rgba16f { self.state_format } else { Format::Rgba8Unorm };
            let side = |v: f32| (v * t.scale * scale).ceil().clamp(1.0, max) as u32;
            let Some(src) = self.borrow((side(rect.w), side(rect.h)), format).map(Src::Pool) else {
                (made..self.made.len()).for_each(|k| self.release(self.made[k]));
                self.made.truncate(made);
                return false;
            };
            self.ops.push(Op::Clear(src, [0.0; 4]));
            self.made.push(src);
        }
        true
    }
}
