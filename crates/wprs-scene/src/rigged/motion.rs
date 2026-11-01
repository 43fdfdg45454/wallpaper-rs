use super::math::{normalized, slerp};
use super::read::Bin;
use super::{MAX_RIG_KEYS, Rigged};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
    Weights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interp {
    Linear,
    Step,
    Cubic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub node: u16,
    pub path: Path,
    pub interp: Interp,
    pub times: Vec<f32>,
    pub values: Vec<f32>,
    pub shapes: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    pub name: String,
    pub length: f32,
    pub channels: Vec<Channel>,
}

impl Channel {
    pub fn width(&self) -> usize {
        match self.path {
            Path::Translation | Path::Scale => 3,
            Path::Rotation => 4,
            Path::Weights => self.shapes.len(),
        }
    }

    pub fn sample(&self, t: f32, out: &mut [f32]) {
        let (n, times) = (self.width(), &self.times);
        let stride = if self.interp == Interp::Cubic { 3 * n } else { n };
        let value = |k: usize, e: usize| {
            let at = k * stride + e + if self.interp == Interp::Cubic { n } else { 0 };
            self.values[at]
        };
        let i = times.partition_point(|x| *x <= t);
        let (a, b) = (i.saturating_sub(1), i.min(times.len() - 1));
        let span = times[b] - times[a];
        let f = if a == b || span <= 0.0 { 0.0 } else { ((t - times[a]) / span).clamp(0.0, 1.0) };
        match self.interp {
            _ if a == b || f == 0.0 => (0..n).for_each(|e| out[e] = value(a, e)),
            Interp::Step => (0..n).for_each(|e| out[e] = value(a, e)),
            Interp::Linear if self.path == Path::Rotation => {
                let q = |k| [value(k, 0), value(k, 1), value(k, 2), value(k, 3)];
                out[..4].copy_from_slice(&slerp(q(a), q(b), f));
            }
            Interp::Linear => (0..n).for_each(|e| out[e] = value(a, e) + (value(b, e) - value(a, e)) * f),
            Interp::Cubic => {
                let tangent =
                    |k: usize, e: usize, side: usize| self.values[k * stride + side * 2 * n + e] * span;
                let (f2, f3) = (f * f, f * f * f);
                for (e, slot) in out.iter_mut().enumerate().take(n) {
                    let (p0, m0, p1, m1) = (value(a, e), tangent(a, e, 1), value(b, e), tangent(b, e, 0));
                    *slot = (2.0 * f3 - 3.0 * f2 + 1.0) * p0
                        + (f3 - 2.0 * f2 + f) * m0
                        + (-2.0 * f3 + 3.0 * f2) * p1
                        + (f3 - f2) * m1;
                }
            }
        }
        if self.path == Path::Rotation {
            let q = normalized([out[0], out[1], out[2], out[3]]);
            out[..4].copy_from_slice(&q);
        }
    }
}

pub(super) fn motions(bin: &Bin, rig: &mut Rigged) -> Result<(), String> {
    let doc = bin.doc;
    let mut total = 0;
    for (i, a) in doc.animations.iter().enumerate() {
        let name = a.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("animation_{i}"));
        let mut motion = Motion { name, length: 0.0, channels: Vec::new() };
        for c in &a.channels {
            let Some(node) = c.target.node.filter(|n| *n < rig.joints.len()) else { continue };
            let path = match c.target.path.as_str() {
                "translation" => Path::Translation,
                "rotation" => Path::Rotation,
                "scale" => Path::Scale,
                "weights" => Path::Weights,
                _ => continue,
            };
            let s = a.samplers.get(c.sampler).ok_or("names a missing animation sampler")?;
            let interp = match s.interpolation.as_deref() {
                Some("STEP") => Interp::Step,
                Some("CUBICSPLINE") => Interp::Cubic,
                _ => Interp::Linear,
            };
            let shapes = match path {
                Path::Weights => weights(bin, rig, node),
                _ => Vec::new(),
            };
            let mut ch = Channel {
                node: node as u16,
                path,
                interp,
                times: bin.floats(s.input, 1)?,
                values: Vec::new(),
                shapes,
            };
            let width = ch.width();
            ch.values = bin.floats(s.output, if path == Path::Weights { 1 } else { width })?;
            let keys = ch.times.len() * width * if interp == Interp::Cubic { 3 } else { 1 };
            let sorted = ch.times.windows(2).all(|w| w[0] <= w[1]);
            if ch.times.is_empty() || ch.values.len() != keys || !sorted || ch.times[0] < 0.0 {
                return Err(format!("animation {i} has keys that are empty, unsorted or of the wrong size"));
            }
            total += keys + ch.times.len();
            if total > MAX_RIG_KEYS {
                return Err(format!("has more than {MAX_RIG_KEYS} animation values"));
            }
            motion.length = motion.length.max(*ch.times.last().unwrap_or(&0.0));
            if width > 0 {
                motion.channels.push(ch);
            }
        }
        rig.motions.push(motion);
    }
    Ok(())
}

fn weights(bin: &Bin, rig: &Rigged, node: usize) -> Vec<u16> {
    let mesh = bin.doc.nodes.get(node).and_then(|n| bin.doc.meshes.get(n.mesh?));
    let Some(mesh) = mesh else { return Vec::new() };
    let count = mesh.primitives.iter().map(|p| p.targets.len()).max().unwrap_or(0);
    let name =
        |t: usize| mesh.extras.as_ref().and_then(|e| e.names.get(t)).cloned().unwrap_or(format!("shape_{t}"));
    (0..count).map(|t| rig.shape(&name(t)).unwrap_or(0) as u16).collect()
}
