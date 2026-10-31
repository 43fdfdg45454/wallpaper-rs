use bytemuck::{Pod, Zeroable};
use wprs_scene::Affine;

mod math;

pub(crate) use math::{
    channel, frame_data, gradient, invert, linear, ndc, premultiplied, rows, scale_of, shape, toned,
};

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
    pub(crate) slots: [[f32; 4]; 24],
    pub(crate) texel: [f32; 4],
    pub(crate) channel: [f32; 4],
    pub(crate) mask_m0: [f32; 4],
    pub(crate) mask_m1: [f32; 4],
    pub(crate) info: [f32; 4],
    pub(crate) area: [f32; 4],
    pub(crate) clip: [f32; 4],
    pub(crate) velocity: [f32; 4],
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

    pub(crate) fn meets(self, o: Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
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

    pub(crate) fn size(self) -> (u32, u32) {
        (self.w.round().max(1.0) as u32, self.h.round().max(1.0) as u32)
    }

    pub(crate) fn affine(self) -> Affine {
        Affine([self.w, 0.0, 0.0, self.h, self.x, self.y])
    }
}
