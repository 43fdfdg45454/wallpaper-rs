use super::Bounds;
use crate::{Affine, Layer, Shape, Value};

pub(super) fn shape(s: &Shape) -> [f32; 2] {
    match *s {
        Shape::Rect { size, .. } | Shape::Ellipse { size } | Shape::Ring { size, .. } => size,
        Shape::Polygon { size, .. } | Shape::Star { size, .. } => [size; 2],
        Shape::Line { to, width } => [to[0].abs() + width, to[1].abs() + width],
    }
}

pub(super) fn fixed<T: Copy>(value: &Value<T>) -> Option<T> {
    match value {
        Value::Fixed(v) => Some(*v),
        Value::Bound(_) => None,
    }
}

pub(super) fn union(a: Bounds, b: Bounds) -> Bounds {
    let ([a0, a1, a2, a3], [b0, b1, b2, b3]) = (a?, b?);
    Some([a0.min(b0), a1.min(b1), a2.max(b2), a3.max(b3)])
}

pub(super) fn quad(layer: &Layer, [w, h]: [f32; 2], m: Affine) -> [f32; 4] {
    let origin = layer.transform.origin.unwrap_or([0.5; 2]);
    let corners = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
        .map(|[u, v]| m.apply([(u - origin[0]) * w, (v - origin[1]) * h]));
    let xs = corners.map(|p| p[0]);
    let ys = corners.map(|p| p[1]);
    let lo = |v: [f32; 4]| v.into_iter().fold(f32::MAX, f32::min);
    let hi = |v: [f32; 4]| v.into_iter().fold(f32::MIN, f32::max);
    [lo(xs), lo(ys), hi(xs), hi(ys)]
}
