use crate::stack::{Info, Layers};
use image::{Rgba, RgbaImage};
use wprs_core::edit::Geometry;

fn at(image: &RgbaImage, x: i64, y: i64) -> [f32; 4] {
    let p = image.get_pixel(
        x.clamp(0, i64::from(image.width()) - 1) as u32,
        y.clamp(0, i64::from(image.height()) - 1) as u32,
    );
    let a = f32::from(p[3]);
    [f32::from(p[0]) * a, f32::from(p[1]) * a, f32::from(p[2]) * a, a]
}

fn sample(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = ((x - x0) as f32, (y - y0) as f32);
    let mut sum = [0.0f32; 4];
    for (dx, dy, w) in
        [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)]
    {
        let p = at(image, x0 as i64 + dx, y0 as i64 + dy);
        sum.iter_mut().zip(p).for_each(|(s, v)| *s += v * w);
    }
    let alpha = sum[3];
    let colour = |v: f32| if alpha > 0.0 { (v / alpha).round().clamp(0.0, 255.0) as u8 } else { 0 };
    Rgba([colour(sum[0]), colour(sum[1]), colour(sum[2]), alpha.round().clamp(0.0, 255.0) as u8])
}

pub fn turned(layers: &Layers, geometry: &Geometry) -> Layers {
    let (w, h) = (layers.info.size[0], layers.info.size[1]);
    let (fw, fh) = geometry.frame((w, h));
    let m = geometry.matrix((w, h));
    let turn = |image: &RgbaImage| {
        RgbaImage::from_fn(fw, fh, |x, y| {
            let (u, v) = ((f64::from(x) + 0.5) / f64::from(fw), (f64::from(y) + 0.5) / f64::from(fh));
            let (sx, sy) = (m[0][0] * u + m[0][1] * v + m[0][2], m[1][0] * u + m[1][1] * v + m[1][2]);
            sample(image, sx * f64::from(w) - 0.5, sy * f64::from(h) - 0.5)
        })
    };
    let images = layers.images.iter().map(turn).collect();
    Layers { info: Info { size: [fw, fh], ..layers.info.clone() }, images }
}
