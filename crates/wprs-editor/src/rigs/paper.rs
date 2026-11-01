use crate::skin::Skin;
use base64::Engine;
use image::{Rgba, RgbaImage};

const SIDE: f32 = 384.0;

pub(super) struct Paper {
    pub heat: RgbaImage,
    pub owner: RgbaImage,
}

pub(super) fn url(image: &RgbaImage) -> String {
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).ok();
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.into_inner()))
}

impl Paper {
    pub fn new(skin: &Skin) -> Paper {
        let [w, h] = skin.size;
        let k = SIDE / w.max(h);
        let (w, h) = ((w * k).round().max(1.0) as u32, (h * k).round().max(1.0) as u32);
        Paper { heat: RgbaImage::new(w, h), owner: RgbaImage::new(w, h) }
    }

    pub fn fill(&mut self, skin: &Skin, t: &[u32; 3], heat: impl Fn([f32; 3]) -> [u8; 3], flat: [u8; 3]) {
        let (w, h) = (self.heat.width() as f32, self.heat.height() as f32);
        let p = t.map(|v| {
            let [x, y] = skin.picture(skin.points[v as usize]);
            [x * w, y * h]
        });
        let edge = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        };
        let area = edge(p[0], p[1], p[2]);
        if !area.is_finite() || area.abs() < 1e-9 {
            return;
        }
        let span = |i: usize, most: f32| {
            let lo = p.iter().map(|q| q[i]).fold(f32::INFINITY, f32::min).floor().max(0.0);
            let hi = p.iter().map(|q| q[i]).fold(f32::NEG_INFINITY, f32::max).ceil().min(most);
            lo as u32..hi as u32
        };
        for y in span(1, h) {
            for x in span(0, w) {
                let c = [x as f32 + 0.5, y as f32 + 0.5];
                let b = [edge(p[1], p[2], c), edge(p[2], p[0], c), edge(p[0], p[1], c)].map(|e| e / area);
                if b.iter().all(|v| *v >= -1e-4) {
                    let [r, g, bl] = heat(b);
                    self.heat.put_pixel(x, y, Rgba([r, g, bl, 255]));
                    self.owner.put_pixel(x, y, Rgba([flat[0], flat[1], flat[2], 255]));
                }
            }
        }
    }
}
