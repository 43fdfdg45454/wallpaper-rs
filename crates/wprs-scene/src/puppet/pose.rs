use super::Puppet;
use crate::resolve::Context;
use crate::rigged::Path;
use crate::rigged::math::{Trs, conjugate, nlerp, normalized, qmul, turn};
use crate::{MeshLayer, sample};

fn mix(local: &mut Trs, rest: &Trs, path: Path, v: &[f32], (w, additive): (f32, bool)) {
    let lerp = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * w);
    match path {
        Path::Translation if additive => (0..3).for_each(|i| local.t[i] += (v[i] - rest.t[i]) * w),
        Path::Translation => local.t = lerp(local.t, [v[0], v[1], v[2]]),
        Path::Rotation => {
            let q = [v[0], v[1], v[2], v[3]];
            local.r = match additive {
                true => normalized(qmul(nlerp([0.0, 0.0, 0.0, 1.0], qmul(q, conjugate(rest.r)), w), local.r)),
                false => nlerp(local.r, q, w),
            }
        }
        Path::Scale if additive => {
            (0..3).for_each(|i| local.s[i] *= 1.0 + (v[i] / rest.s[i].max(1e-6) - 1.0) * w)
        }
        Path::Scale => local.s = lerp(local.s, [v[0], v[1], v[2]]),
        Path::Weights => {}
    }
}

impl Puppet {
    pub(crate) fn blend(&mut self, cx: &Context, m: &MeshLayer) {
        let Puppet { heads, clips, locals, shape, sorted, buffer, rig, .. } = self;
        locals.clear();
        locals.extend(rig.joints.iter().map(|j| j.rest));
        shape.clear();
        shape.extend_from_slice(&rig.defaults);
        sorted.clear();
        sorted.resize(rig.parts.len(), None);
        for additive in [false, true] {
            for h in heads.iter() {
                let a = h.layer.map(|i| &m.rig.animations[i]);
                if a.is_some_and(|a| a.additive) != additive {
                    continue;
                }
                let w =
                    h.weight.unwrap_or_else(|| a.map_or(1.0, |a| a.weight.get(cx))).clamp(0.0, 1.0) * h.fade;
                if w <= 0.0 {
                    continue;
                }
                let (clip, map) = (&m.rig.clips[h.clip], &clips[h.clip]);
                let t = crate::local(h.t as f32, map.length, clip.repeat) + clip.from;
                let open = |n: usize| h.mask.is_empty() || h.mask[n];
                for ch in map.motion.map_or(&[][..], |i| &rig.motions[i].channels[..]) {
                    let n = ch.node as usize;
                    ch.sample(t, buffer);
                    match ch.path {
                        Path::Weights => ch.shapes.iter().enumerate().for_each(|(e, s)| {
                            let s = *s as usize;
                            shape[s] = weigh(shape[s], buffer[e], rig.defaults[s], (w, additive));
                        }),
                        path if open(n) => {
                            mix(&mut locals[n], &rig.joints[n].rest, path, buffer, (w, additive))
                        }
                        _ => {}
                    }
                }
                for ((_, keys), node) in clip.bones.iter().zip(&map.bones) {
                    let Some(n) = node.map(usize::from).filter(|n| open(*n)) else { continue };
                    let rest = rig.joints[n].rest;
                    if !keys.rotation.is_empty() {
                        let q = qmul(rest.r, turn(sample(&keys.rotation, t)));
                        mix(&mut locals[n], &rest, Path::Rotation, &q, (w, additive));
                    }
                    if !keys.position.is_empty() {
                        let [x, y] = sample(&keys.position, t).0;
                        let v = [rest.t[0] + x, rest.t[1] - y, rest.t[2]];
                        mix(&mut locals[n], &rest, Path::Translation, &v, (w, additive));
                    }
                    if !keys.scale.is_empty() {
                        let [x, y] = sample(&keys.scale, t).0;
                        let v = [rest.s[0] * x, rest.s[1] * y, rest.s[2]];
                        mix(&mut locals[n], &rest, Path::Scale, &v, (w, additive));
                    }
                }
                for ((_, keys), s) in clip.shapes.iter().zip(&map.shapes) {
                    let Some(s) = s.map(usize::from) else { continue };
                    shape[s] = weigh(shape[s], sample(keys, t), rig.defaults[s], (w, additive));
                }
                for ((_, keys), p) in clip.order.iter().zip(&map.order) {
                    if let Some(p) = p {
                        sorted[*p as usize] = Some(sample(keys, t).round());
                    }
                }
            }
        }
        for (name, v) in &m.rig.shapes {
            let x = v.get(cx);
            match m.rig.expressions.get(name) {
                Some(parts) => parts.iter().for_each(|(s, k)| {
                    rig.shape(s).into_iter().for_each(|i| shape[i] += k * x);
                }),
                None => rig.shape(name).into_iter().for_each(|i| shape[i] += x),
            }
        }
        shape.iter_mut().for_each(|s| *s = s.clamp(-1.0, 1.0));
    }
}

fn weigh(now: f32, v: f32, default: f32, (w, additive): (f32, bool)) -> f32 {
    if additive { now + (v - default) * w } else { now + (v - now) * w }
}
