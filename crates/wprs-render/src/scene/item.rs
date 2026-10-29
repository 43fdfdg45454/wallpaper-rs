use bytemuck::{Pod, Zeroable};
use wprs_scene::{Affine, Color, Frame, Gradient, MaskChannel, Quality, Runtime, Shape};

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub(crate) struct Item {
    pub(crate) m0: [f32; 4],
    pub(crate) m1: [f32; 4],
    pub(crate) quad: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) uv: [f32; 4],
    pub(crate) size: [f32; 4],
    pub(crate) shape: [f32; 4],
    pub(crate) place: [f32; 4],
    pub(crate) stroke: [f32; 4],
    pub(crate) style: [f32; 4],
    pub(crate) grad: [f32; 4],
    pub(crate) stops: [f32; 4],
    pub(crate) stop_colors: [[f32; 4]; 4],
    pub(crate) channel: [f32; 4],
    pub(crate) mask_info: [f32; 4],
    pub(crate) mask_m0: [f32; 4],
    pub(crate) mask_m1: [f32; 4],
    pub(crate) mask_shape: [f32; 4],
    pub(crate) back: [f32; 4],
    pub(crate) mode: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Fx {
    pub(crate) slots: [[f32; 4]; 16],
    pub(crate) texel: [f32; 4],
    pub(crate) channel: [f32; 4],
    pub(crate) mask_m0: [f32; 4],
    pub(crate) mask_m1: [f32; 4],
    pub(crate) info: [f32; 4],
    pub(crate) area: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct FrameData {
    pub(crate) time: f32,
    pub(crate) dt: f32,
    pub(crate) resolution: [f32; 2],
    pub(crate) cursor: [f32; 2],
    pub(crate) cursor_delta: [f32; 2],
    pub(crate) bass: f32,
    pub(crate) mid: f32,
    pub(crate) treble: f32,
    pub(crate) level: f32,
    pub(crate) beat: f32,
    pub(crate) srgb: f32,
    pub(crate) quality: f32,
    pub(crate) seed: f32,
    pub(crate) buttons: f32,
    pub(crate) inside: f32,
    pub(crate) since_beat: f32,
    pub(crate) presence: f32,
    pub(crate) audio: [[f32; 4]; 16],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

impl Rect {
    pub(crate) fn around(points: impl IntoIterator<Item = [f32; 2]>) -> Option<Rect> {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for [x, y] in points {
            (lo, hi) = ([lo[0].min(x), lo[1].min(y)], [hi[0].max(x), hi[1].max(y)]);
        }
        (lo[0] <= hi[0]).then(|| Rect { x: lo[0], y: lo[1], w: hi[0] - lo[0], h: hi[1] - lo[1] })
    }

    pub(crate) fn union(self, o: Rect) -> Rect {
        Rect::around([
            [self.x, self.y],
            [self.x + self.w, self.y + self.h],
            [o.x, o.y],
            [o.x + o.w, o.y + o.h],
        ])
        .unwrap_or(self)
    }

    pub(crate) fn grow(self, by: f32) -> Rect {
        Rect { x: self.x - by, y: self.y - by, w: self.w + 2.0 * by, h: self.h + 2.0 * by }
    }

    pub(crate) fn pixels(self, bounds: Rect) -> Option<Rect> {
        let (x0, y0) = (self.x.max(bounds.x).floor(), self.y.max(bounds.y).floor());
        let x1 = (self.x + self.w).min(bounds.x + bounds.w).ceil();
        let y1 = (self.y + self.h).min(bounds.y + bounds.h).ceil();
        (x1 > x0 && y1 > y0).then_some(Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
    }

    pub(crate) fn bucket(self) -> Rect {
        let up = |v: f32| (v / 64.0).ceil() * 64.0;
        Rect { w: up(self.w), h: up(self.h), ..self }
    }

    pub(crate) fn size(self) -> (u32, u32) {
        (self.w.round().max(1.0) as u32, self.h.round().max(1.0) as u32)
    }

    pub(crate) fn affine(self) -> Affine {
        Affine([self.w, 0.0, 0.0, self.h, self.x, self.y])
    }
}

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
