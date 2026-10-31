use super::{FrameData, Item, Rect};
use wprs_scene::{Affine, Color, Frame, Gradient, MaskChannel, Quality, Runtime, Shape};

pub(crate) fn rows(a: Affine) -> ([f32; 4], [f32; 4]) {
    let [a, b, c, d, e, f] = a.0;
    ([a, c, e, 0.0], [b, d, f, 0.0])
}

pub(crate) fn ndc(target: Rect, m: Affine) -> ([f32; 4], [f32; 4]) {
    let to = Affine([
        2.0 / target.w,
        0.0,
        0.0,
        -2.0 / target.h,
        -1.0 - 2.0 * target.x / target.w,
        1.0 + 2.0 * target.y / target.h,
    ]);
    rows(to.then(m))
}

pub(crate) fn invert(m: Affine) -> Affine {
    let [a, b, c, d, e, f] = m.0;
    let det = a * d - b * c;
    if det.abs() < 1e-12 {
        return Affine([0.0; 6]);
    }
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    Affine([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)])
}

pub(crate) fn scale_of(m: Affine) -> f32 {
    let [a, b, c, d, ..] = m.0;
    (a * d - b * c).abs().sqrt()
}

pub(crate) fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub(crate) fn premultiplied(Color([r, g, b, a]): Color, srgb: bool, opacity: f32) -> [f32; 4] {
    let conv = |c: f32| if srgb { c } else { linear(c) };
    let a = a * opacity;
    [conv(r) * a, conv(g) * a, conv(b) * a, a]
}

pub(crate) fn toned(tint: Color, brightness: f32, srgb: bool, opacity: f32) -> [f32; 4] {
    let [r, g, b, a] = premultiplied(tint, srgb, opacity);
    [r * brightness, g * brightness, b * brightness, a]
}

pub(crate) fn channel(channel: Option<MaskChannel>, texture: bool) -> [f32; 4] {
    match channel.unwrap_or(if texture { MaskChannel::Luminance } else { MaskChannel::A }) {
        MaskChannel::R => [1.0, 0.0, 0.0, 0.0],
        MaskChannel::G => [0.0, 1.0, 0.0, 0.0],
        MaskChannel::B => [0.0, 0.0, 1.0, 0.0],
        MaskChannel::A => [0.0, 0.0, 0.0, 1.0],
        MaskChannel::Luminance => [0.2126, 0.7152, 0.0722, 0.0],
    }
}

pub(crate) fn shape(s: &Shape) -> ([f32; 2], [f32; 4], [f32; 2]) {
    let centered =
        |size: [f32; 2], kind: f32, a: f32, b: f32| (size, [kind, a, b, 0.0], [size[0] / 2.0, size[1] / 2.0]);
    match *s {
        Shape::Rect { size, radius } => centered(size, 0.0, radius, 0.0),
        Shape::Ellipse { size } => centered(size, 1.0, 0.0, 0.0),
        Shape::Ring { size, width } => centered(size, 2.0, width, 0.0),
        Shape::Polygon { sides, size } => centered([size; 2], 3.0, sides as f32, 0.0),
        Shape::Star { points, size, inner } => centered([size; 2], 4.0, points as f32, inner),
        Shape::Line { to, width } => {
            let size = [to[0].abs() + width, to[1].abs() + width];
            let place = [width / 2.0 - to[0].min(0.0), width / 2.0 - to[1].min(0.0)];
            (size, [5.0, width, to[0], to[1]], place)
        }
    }
}

pub(crate) fn gradient(item: &mut Item, g: &Gradient, srgb: bool) {
    let n = g.stops.len().min(4);
    item.style[2] = if g.radial { 2.0 } else { 1.0 };
    item.style[3] = n as f32;
    item.grad = [g.from[0], g.from[1], g.to[0], g.to[1]];
    for (i, (at, color)) in g.stops.iter().take(4).enumerate() {
        item.stops[i] = *at;
        item.stop_colors[i] = premultiplied(*color, srgb, 1.0);
    }
}

pub(crate) fn frame_data(
    runtime: &Runtime,
    frame: &Frame<'_>,
    size: (u32, u32),
    srgb: bool,
    quality: Quality,
) -> FrameData {
    let (audio, pointer) = (runtime.audio(), runtime.pointer());
    let bands: [[f32; 4]; 16] = std::array::from_fn(|i| std::array::from_fn(|j| audio.average[i * 4 + j]));
    FrameData {
        time: frame.time,
        dt: runtime.dt(),
        resolution: [size.0 as f32, size.1 as f32],
        cursor: pointer.smooth,
        cursor_delta: pointer.velocity,
        bass: audio.bass,
        mid: audio.mid,
        treble: audio.treble,
        level: audio.level,
        beat: f32::from(u8::from(audio.beat)),
        srgb: f32::from(u8::from(srgb)),
        quality: quality as u8 as f32,
        seed: (runtime.seed % 65_536) as f32,
        buttons: f32::from(pointer.buttons),
        inside: f32::from(u8::from(pointer.inside)),
        since_beat: runtime.since_beat(),
        presence: pointer.presence * pointer.presence * (3.0 - 2.0 * pointer.presence),
        audio: bands,
    }
}
