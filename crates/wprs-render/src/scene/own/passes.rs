use super::super::fed::Fed;
use super::super::item::Rect;
use super::super::paint::Painter;
use super::super::pipes::Shader;
use super::super::stops::More;
use super::super::{Fill, Op, Src};
use super::{AT, Inputs, mode, target};
use wgpu::TextureFormat as Format;
use wprs_scene::{Descriptor, EffectDraw, PassDef, Texel};

impl Painter<'_, '_, '_> {
    pub(super) fn input(
        &mut self,
        e: &EffectDraw<'_>,
        fed: &Fed<'_>,
        at: &Inputs,
        name: &str,
    ) -> Option<Src> {
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

    pub(super) fn record(
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

    pub(super) fn unclear(&mut self, target: Src, from: usize) -> bool {
        let fresh = |op: &Op| matches!(op, Op::Clear(t, _) if *t == target);
        let Some(at) = self.ops[from..].iter().position(fresh).map(|k| from + k) else { return false };
        let used = self.ops[at + 1..].iter().any(|op| match op {
            Op::Effect(f) => f.target == target || self.sets[f.tex as usize].contains(&target),
            Op::Clear(..) => false,
            _ => true,
        });
        if !used {
            self.ops.remove(at);
        }
        !used
    }

    pub(super) fn targets(&mut self, d: &Descriptor, rect: Rect, scale: f32) -> bool {
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
