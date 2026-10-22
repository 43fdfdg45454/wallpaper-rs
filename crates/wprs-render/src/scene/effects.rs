use super::Op;
use super::Src;
use super::item::{Fx, Rect, channel, rows, scale_of};
use super::paint::Painter;
use wprs_scene::{Affine, MaskMode};

impl Painter<'_, '_, '_> {
    fn pass(&mut self, entry: &'static str, rect: Rect, scale: f32, inputs: &[Src], fx: Fx) -> Src {
        let size = ((rect.w * scale).ceil().max(1.0) as u32, (rect.h * scale).ceil().max(1.0) as u32);
        let first = inputs.first().copied().unwrap_or(Src::Clear);
        let Some(out) = self.acquire(size) else { return first };
        let (w, h) = self.size_of(first);
        let px = scale_of(self.screen) * w as f32 / rect.w.max(1.0);
        let fx = Fx {
            texel: [1.0 / w as f32, 1.0 / h as f32, w as f32, h as f32],
            info: [fx.info[0], 0.0, fx.info[2], px],
            ..fx
        };
        let slot = self.pool.push(&fx);
        let mut tex = [Src::Clear; 6];
        tex[0] = first;
        tex[1] = inputs.get(1).copied().unwrap_or(Src::White);
        inputs.iter().skip(2).take(4).enumerate().for_each(|(i, s)| tex[2 + i] = *s);
        self.ops.push(Op::Effect { target: out, entry, slot, tex });
        out
    }

    pub(crate) fn effects(&mut self, i: usize, input: Src, rect: Rect, layer_uv: Affine) -> Src {
        let (frame, images) = (self.frame, self.images);
        let d = &frame.draws[i];
        let (source, mut current) = (input, input);
        for e in &frame.effects[d.effects.clone()] {
            let mask =
                e.mask.and_then(|m| m.texture.as_ref()).and_then(|id| images.get(id)).map(|(image, _)| image);
            let mask = mask.map_or(Src::White, |image| self.ext(image));
            let (m0, m1) = rows(layer_uv);
            let invert = e.mask.is_some_and(|m| m.invert);
            let fx = Fx {
                slots: e.slots,
                texel: [0.0; 4],
                channel: e.mask.map_or([0.0, 0.0, 0.0, 1.0], |m| channel(m.channel, m.texture.is_some())),
                mask_m0: m0,
                mask_m1: m1,
                info: [f32::from(u8::from(invert)), 0.0, 0.0, 0.0],
            };
            let mut outputs: Vec<Src> = Vec::with_capacity(e.descriptor.passes.len());
            let mut previous = current;
            for (n, pass) in e.descriptor.passes.iter().enumerate() {
                let inputs: Vec<Src> = pass
                    .inputs
                    .iter()
                    .map(|name| match name.as_str() {
                        "previous" => previous,
                        "input" => current,
                        "source" => source,
                        "mask" => mask,
                        other => other
                            .strip_prefix("pass:")
                            .and_then(|k| outputs.get(k.parse::<usize>().ok()?))
                            .copied()
                            .unwrap_or(Src::Clear),
                    })
                    .collect();
                let inputs: Vec<Src> =
                    [inputs[0], mask].into_iter().chain(inputs.into_iter().skip(1)).collect();
                let info = [fx.info[0], 0.0, n as f32, 0.0];
                previous = self.pass(
                    pass.entry.as_str(),
                    rect,
                    (pass.scale * e.scale).clamp(0.05, 1.0),
                    &inputs,
                    Fx { info, ..fx },
                );
                outputs.push(previous);
            }
            if e.descriptor.mask_mode == MaskMode::Mix && e.mask.is_some() {
                previous = self.pass("fs_mask_mix", rect, 1.0, &[previous, mask, current], fx);
            }
            current = previous;
        }
        current
    }
}
