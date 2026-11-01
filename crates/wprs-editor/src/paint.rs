use crate::layers::unique;
use crate::ops::{Op, escape, set};
use anyhow::{Result, ensure};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Param, Scene};

const SIDE: u32 = 512;
const POINTS: usize = 4096;

pub struct Stroke<'a> {
    pub pointer: &'a str,
    pub channel: Option<usize>,
    pub radius: f32,
    pub hardness: f32,
    pub value: f32,
    pub points: &'a [[f32; 2]],
}

pub(crate) fn paint(dir: &Path, doc: &Value, s: &Stroke) -> Result<Vec<Op>> {
    let param =
        s.pointer.starts_with("/scene/") && s.pointer.contains("/effects/") && s.pointer.contains("/params/");
    ensure!(param, said("editor.paint_param", "masks are painted on a parameter of an effect", &[]));
    let fits = (1..=POINTS).contains(&s.points.len()) && s.channel.is_none_or(|c| c < 4);
    ensure!(
        fits,
        said("editor.paint_stroke", "a stroke has 1 to %1 points and a channel from 0 to 3", &[&POINTS])
    );
    let asset = |id: &str| format!("/scene/assets/{}", escape(id));
    let named = doc.pointer(s.pointer).and_then(Value::as_str).filter(|id| doc.pointer(&asset(id)).is_some());
    let id = named.map_or_else(|| unique(doc, s.pointer.rsplit('/').next().unwrap_or("mask")), String::from);
    let path = named.and_then(|id| doc.pointer(&format!("{}/path", asset(id)))?.as_str());
    let old = path.and_then(|p| image::open(dir.join(p)).ok()).map(|i| i.into_rgba8());
    let mut picture =
        old.unwrap_or_else(|| RgbaImage::from_pixel(SIDE, SIDE, Rgba(blank(dir, doc, s.pointer))));
    stroke(&mut picture, s);
    let saved =
        (1..).map(|n| format!("masks/{id}-{n}.png")).find(|p| !dir.join(p).exists()).unwrap_or_default();
    std::fs::create_dir_all(dir.join("masks"))?;
    picture.save(dir.join(&saved))?;
    let mut ops = set(doc, &asset(&id), json!({ "path": saved, "linear": true }));
    ops.extend(set(doc, s.pointer, json!(id)));
    Ok(ops)
}

fn blank(dir: &Path, doc: &Value, pointer: &str) -> [u8; 4] {
    let painted = || {
        let (at, id) = pointer.rsplit_once("/params/")?;
        let name = doc.pointer(&format!("{at}/effect"))?.as_str()?;
        let mut scene = serde_json::from_value::<Scene>(doc["scene"].clone()).ok()?;
        crate::cache::read_files(&mut scene, dir);
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        match scene.effect(name)?.params.iter().find(|p| p.id == id)?.paint.as_ref()? {
            Param::Color(c) => Some(c.0.map(byte)),
            Param::Number(n) => Some([byte(*n); 4]),
            _ => None,
        }
    };
    painted().unwrap_or([0, 0, 0, 255])
}

fn stroke(picture: &mut RgbaImage, s: &Stroke) {
    let (w, h) = picture.dimensions();
    let r = s.radius.clamp(0.001, 0.5) * w.min(h) as f32;
    let soft = r * s.hardness.clamp(0.0, 0.99);
    let mut cover = vec![0.0f32; (w * h) as usize];
    let first = s.points[0];
    let pairs = s.points.windows(2).map(|p| (p[0], p[1])).chain([(first, first)]);
    for (a, b) in pairs {
        let (a, b) = ([a[0] * w as f32, a[1] * h as f32], [b[0] * w as f32, b[1] * h as f32]);
        let (x0, x1) = ((a[0].min(b[0]) - r).max(0.0) as u32, ((a[0].max(b[0]) + r).ceil() as u32).min(w));
        let (y0, y1) = ((a[1].min(b[1]) - r).max(0.0) as u32, ((a[1].max(b[1]) + r).ceil() as u32).min(h));
        let ab = [b[0] - a[0], b[1] - a[1]];
        let long = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-6);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = [x as f32 + 0.5 - a[0], y as f32 + 0.5 - a[1]];
                let t = ((p[0] * ab[0] + p[1] * ab[1]) / long).clamp(0.0, 1.0);
                let d = (p[0] - ab[0] * t).hypot(p[1] - ab[1] * t);
                let k = 1.0 - ((d - soft) / (r - soft).max(1e-3)).clamp(0.0, 1.0);
                let c = &mut cover[(y * w + x) as usize];
                *c = c.max(k * k * (3.0 - 2.0 * k));
            }
        }
    }
    let value = s.value.clamp(0.0, 1.0) * 255.0;
    let channels = s.channel.map_or(0..3, |c| c..c + 1);
    for (pixel, k) in picture.pixels_mut().zip(cover).filter(|(_, k)| *k > 0.0) {
        for c in channels.clone() {
            let old = f32::from(pixel.0[c]);
            pixel.0[c] = (old + (value - old) * k).round() as u8;
        }
    }
}
