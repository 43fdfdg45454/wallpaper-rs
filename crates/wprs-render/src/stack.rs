use crate::color::{balance, brush, curves};
use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use wprs_core::config::parse_hex_color;
use wprs_core::edit::{Edit, Kind, Mask, Matrix, Stroke};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Op {
    kind: u32,
    mask: u32,
    mode: u32,
    opacity: f32,
    a: [f32; 4],
    m: [[f32; 4]; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Gate {
    kind: u32,
    invert: u32,
    channel: u32,
    pad: u32,
    a: [f32; 4],
    b: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Stack {
    geo: [[f32; 4]; 3],
    pub(crate) start: u32,
    pub(crate) end: u32,
    aspect: f32,
    ratio: f32,
    ops: [Op; 24],
    masks: [Gate; 8],
}

pub(crate) struct Plan {
    pub(crate) stack: Stack,
    pub(crate) lut: (Vec<u16>, u32),
    pub(crate) brush: Option<RgbaImage>,
    pub(crate) blurs: Vec<(u32, f32)>,
}

fn columns(m: &Matrix) -> [[f32; 4]; 3] {
    std::array::from_fn(|j| [m[0][j] as f32, m[1][j] as f32, m[2][j] as f32, 0.0])
}

fn gate(mask: &Mask, brushes: &mut u32, deep: bool) -> Gate {
    let mut g = Gate::zeroed();
    let invert = match *mask {
        Mask::Linear { from, to, invert } => {
            (g.kind, g.a) = (1, [from[0], from[1], to[0], to[1]]);
            invert
        }
        Mask::Radial { center, radius, feather, invert } => {
            (g.kind, g.a, g.b[0]) = (2, [center[0], center[1], radius[0], radius[1]], feather.max(1e-3));
            invert
        }
        Mask::Luminance { range, soft, invert } => {
            (g.kind, g.a) = (3, [range[0], range[1], soft.max(1e-3), 0.0]);
            invert
        }
        Mask::Depth { range, soft, invert } => {
            (g.kind, g.a) = (if deep { 5 } else { 6 }, [range[0], range[1], soft.max(1e-3), 0.0]);
            invert
        }
        Mask::Brush { invert, .. } => {
            (g.kind, g.channel, *brushes) = (4, *brushes, *brushes + 1);
            invert
        }
    };
    g.invert = u32::from(invert);
    g
}

pub(crate) fn plan(edit: &Edit, image: (u32, u32), size: (u32, u32), deep: bool) -> Plan {
    let mut stack = Stack::zeroed();
    stack.geo = columns(&edit.geometry.matrix(image));
    let ratio = |(w, h): (u32, u32)| w.max(1) as f32 / h.max(1) as f32;
    (stack.aspect, stack.ratio) = (ratio(image), ratio(size));
    let mut brushes = 0;
    for (i, mask) in edit.masks.values().enumerate() {
        stack.masks[i] = gate(mask, &mut brushes, deep);
    }
    let (mut lut, mut rows, mut blurs, mut count) = (Vec::new(), 0, Vec::new(), 0);
    let long = size.0.max(size.1) as f32;
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
    for op in edit.ops.iter().filter(|op| op.enabled) {
        let mask =
            op.mask.as_ref().and_then(|m| edit.masks.keys().position(|k| k == m)).map_or(0, |i| i as u32 + 1);
        let mut o =
            Op { kind: 0, mask, mode: 0, opacity: op.opacity.unwrap_or(1.0), a: [0.0; 4], m: identity };
        match &op.kind {
            Kind::Exposure { ev } => (o.kind, o.a[0]) = (1, *ev),
            Kind::WhiteBalance { temperature, tint } => {
                (o.kind, o.m) = (2, columns(&balance(*temperature, *tint)))
            }
            Kind::Contrast { amount } => (o.kind, o.a[0]) = (3, 1.5f32.powf(amount / 100.0)),
            Kind::Saturation { saturation, vibrance } => {
                (o.kind, o.a[0], o.a[1]) = (4, 1.0 + saturation / 100.0, vibrance / 100.0)
            }
            Kind::Curves { rgb, r, g, b } => {
                lut.extend(curves([r, g, b, rgb].map(Vec::as_slice)));
                (o.kind, o.mode, rows) = (5, rows, rows + 1)
            }
            Kind::Overlay { color, blend } => {
                let [r, g, b] = parse_hex_color(color).unwrap_or_default().map(|c| f32::from(c) / 255.0);
                (o.kind, o.mode, o.a, o.opacity) = (6, *blend as u32, [r, g, b, o.opacity], 1.0)
            }
            Kind::Vignette { amount, midpoint, roundness, feather } => {
                (o.kind, o.a) = (7, [*amount, *midpoint, *roundness, feather.max(1e-3)])
            }
            Kind::Blur { radius } if radius * long / 1000.0 >= 0.5 => {
                blurs.push((count, radius * long / 1000.0));
                o.kind = 8
            }
            Kind::Blur { .. } => continue,
        }
        stack.ops[count as usize] = o;
        count += 1;
    }
    stack.end = count;
    let strokes: Vec<&[Stroke]> = (edit.masks.values())
        .filter_map(|m| match m {
            Mask::Brush { strokes, .. } => Some(strokes.as_slice()),
            _ => None,
        })
        .collect();
    let brush = (!strokes.is_empty()).then(|| brush(&strokes, stack.aspect));
    Plan { stack, lut: (lut, rows), brush, blurs }
}
