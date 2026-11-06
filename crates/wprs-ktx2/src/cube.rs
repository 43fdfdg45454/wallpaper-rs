use crate::{Error, MAX_SIDE};
use image::{Rgba, Rgba32FImage, RgbaImage};
use std::f32::consts::PI;

fn toward(face: u32, s: f32, t: f32) -> [f32; 3] {
    let (a, b) = (2.0 * s - 1.0, 2.0 * t - 1.0);
    match face {
        0 => [1.0, -b, -a],
        1 => [-1.0, -b, a],
        2 => [a, 1.0, b],
        3 => [a, -1.0, -b],
        4 => [a, -b, 1.0],
        _ => [-a, -b, -1.0],
    }
}

fn sample(panorama: &Rgba32FImage, [x, y, z]: [f32; 3]) -> Rgba<f32> {
    let (w, h) = panorama.dimensions();
    let long = (x * x + y * y + z * z).sqrt().max(1e-6);
    let (lon, lat) = (x.atan2(z), (y / long).clamp(-1.0, 1.0).asin());
    let u = (lon + PI) / (2.0 * PI) * w as f32 - 0.5;
    let v = ((PI / 2.0 - lat) / PI * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (u0, v0) = (u.floor(), v.floor());
    let (fu, fv) = (u - u0, v - v0);
    let column = |c: f32| (c as i64).rem_euclid(i64::from(w)) as u32;
    let row = |r: f32| (r as u32).min(h - 1);
    let at = |c: f32, r: f32| panorama.get_pixel(column(c), row(r)).0;
    let (p00, p10, p01, p11) = (at(u0, v0), at(u0 + 1.0, v0), at(u0, v0 + 1.0), at(u0 + 1.0, v0 + 1.0));
    Rgba(std::array::from_fn(|k| {
        let top = p00[k] + (p10[k] - p00[k]) * fu;
        let bottom = p01[k] + (p11[k] - p01[k]) * fu;
        top + (bottom - top) * fv
    }))
}

pub fn faces(panorama: &Rgba32FImage) -> Result<Vec<Rgba32FImage>, Error> {
    let (w, h) = panorama.dimensions();
    if w < 4 || h < 2 {
        return Err(Error::Broken);
    }
    let side = (w / 4).min(MAX_SIDE);
    let face = |f: u32| {
        Rgba32FImage::from_fn(side, side, |x, y| {
            let (s, t) = ((x as f32 + 0.5) / side as f32, (y as f32 + 0.5) / side as f32);
            sample(panorama, toward(f, s, t))
        })
    };
    Ok((0..6).map(face).collect())
}

pub fn plain_faces(panorama: &RgbaImage) -> Result<Vec<RgbaImage>, Error> {
    let wide = image::DynamicImage::ImageRgba8(panorama.clone()).into_rgba32f();
    let made = faces(&wide)?;
    Ok(made.into_iter().map(|f| image::DynamicImage::ImageRgba32F(f).into_rgba8()).collect())
}
