use super::lit::{lit_layer, picture};
use crate::action::{MapStroke, NormalsMake};
use crate::ops::{Op, escape, set};
use crate::paint::{POINTS, Stroke, stroke};
use anyhow::{Result, anyhow, ensure};
use image::{Rgba, RgbaImage, imageops};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

const SHOWN: u32 = 256;
const SIDE: u32 = 512;
const LIGHT: [f32; 3] = [-1.0, 1.0, 1.2];

fn path_of<'a>(doc: &'a Value, id: &str) -> Option<&'a str> {
    doc.pointer(&format!("/scene/assets/{}/path", escape(id))).and_then(Value::as_str)
}

fn source(doc: &Value, m: &NormalsMake) -> Result<String> {
    let from_layer = || -> Option<String> {
        let (_, l) = crate::actions::layer(doc, m.layer.as_deref()?).ok()?;
        l["image"].as_str().map(String::from)
    };
    let id = m.asset.clone().or_else(from_layer);
    let id =
        id.ok_or_else(|| anyhow!(said("editor.normals_from", "name a picture or an image layer", &[])))?;
    picture(doc, &id)?;
    Ok(id)
}

fn made(dir: &Path, doc: &Value, m: &NormalsMake) -> Result<(String, RgbaImage)> {
    let fits = |v: f32, hi: f32| v.is_finite() && (0.0..=hi).contains(&v);
    ensure!(
        fits(m.strength, wprs_depth::MAX_STRENGTH) && fits(m.blur, wprs_depth::MAX_BLUR),
        said(
            "editor.normals_range",
            "strength goes from 0 to %1 and blur from 0 to %2",
            &[&wprs_depth::MAX_STRENGTH, &wprs_depth::MAX_BLUR]
        )
    );
    let id = source(doc, m)?;
    let picture = image::open(dir.join(path_of(doc, &id).unwrap_or_default()))?.into_rgba8();
    Ok((id, wprs_depth::normals(&picture, m.strength, m.blur)))
}

fn saved(dir: &Path, folder: &str, name: &str, image: &RgbaImage) -> Result<String> {
    let path =
        (1..).map(|n| format!("{folder}/{name}-{n}.png")).find(|p| !dir.join(p).exists()).unwrap_or_default();
    std::fs::create_dir_all(dir.join(folder))?;
    image.save(dir.join(&path))?;
    Ok(path)
}

fn with(doc: &Value, layer: &str, field: &str, id: &str) -> Result<Vec<Op>> {
    let (path, l) = lit_layer(doc, layer)?;
    let mut block = l["lighting"].as_object().cloned().unwrap_or_default();
    block.insert(field.into(), json!(id));
    Ok(set(doc, &format!("{path}/lighting"), Value::Object(block)))
}

pub(crate) fn normals(dir: &Path, doc: &Value, m: &NormalsMake) -> Result<Vec<Op>> {
    if let Some(layer) = &m.layer {
        lit_layer(doc, layer)?;
    }
    let (from, image) = made(dir, doc, m)?;
    let base =
        m.id.clone().unwrap_or_else(|| format!("{}_normals", from.chars().take(24).collect::<String>()));
    let id = if m.id.is_some() { base } else { crate::layers::unique(doc, &base) };
    let path = saved(dir, "normals", &id, &image)?;
    let mut ops =
        set(doc, &format!("/scene/assets/{}", escape(&id)), json!({ "path": path, "linear": true }));
    if let Some(layer) = &m.layer {
        ops.extend(with(doc, layer, "normal", &id)?);
    }
    Ok(ops)
}

fn small(image: &RgbaImage) -> RgbaImage {
    let k = SHOWN as f32 / image.width().max(image.height()) as f32;
    if k >= 1.0 {
        return image.clone();
    }
    let (w, h) = ((image.width() as f32 * k).max(1.0) as u32, (image.height() as f32 * k).max(1.0) as u32);
    imageops::resize(image, w, h, imageops::FilterType::Triangle)
}

pub fn preview(dir: &Path, doc: &Value, m: &NormalsMake) -> Value {
    let Ok((from, image)) = made(dir, doc, m) else { return Value::Null };
    let shown = small(&image);
    json!({ "from": from, "size": [shown.width(), shown.height()], "normals": crate::rigs::url(&shown),
        "relief": crate::rigs::url(&wprs_depth::relief(&shown, LIGHT)),
        "sphere": crate::rigs::url(&wprs_depth::sphere(96, LIGHT)) })
}

fn blank(dir: &Path, doc: &Value, layer: &Value, white: bool) -> RgbaImage {
    let found = layer["image"]
        .as_str()
        .and_then(|id| path_of(doc, id))
        .and_then(|p| image::image_dimensions(dir.join(p)).ok());
    let (w, h) = found.map_or((SIDE, SIDE), |(w, h)| {
        let k = (SIDE as f32 / w.max(h) as f32).min(1.0);
        (((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1))
    });
    let v = if white { 255 } else { 0 };
    RgbaImage::from_pixel(w, h, Rgba([v, v, v, 255]))
}

pub(crate) fn paint(dir: &Path, doc: &Value, s: &MapStroke) -> Result<Vec<Op>> {
    let (_, layer) = lit_layer(doc, &s.layer)?;
    let fits = (1..=POINTS).contains(&s.points.len()) && s.points.iter().flatten().all(|v| v.is_finite());
    ensure!(
        fits,
        said("editor.paint_stroke", "a stroke has 1 to %1 points and a channel from 0 to 3", &[&POINTS])
    );
    let field = s.map.field();
    let named = layer["lighting"][field].as_str().filter(|id| path_of(doc, id).is_some());
    let old = named.and_then(|id| image::open(dir.join(path_of(doc, id)?)).ok()).map(|i| i.into_rgba8());
    let mut picture = old.unwrap_or_else(|| blank(dir, doc, layer, field == "roughness_map"));
    let pointer = format!("/scene/layers/{}", s.layer);
    let brush = Stroke {
        pointer: &pointer,
        channel: None,
        radius: s.radius,
        hardness: s.hardness,
        value: s.value,
        points: &s.points,
    };
    stroke(&mut picture, &brush);
    let id = named.map_or_else(
        || crate::layers::unique(doc, &format!("{}_{}", s.layer, &field[..field.len() - 4])),
        String::from,
    );
    let path = saved(dir, "maps", &id, &picture)?;
    let mut ops =
        set(doc, &format!("/scene/assets/{}", escape(&id)), json!({ "path": path, "linear": true }));
    ops.extend(with(doc, &s.layer, field, &id)?);
    Ok(ops)
}

pub fn heat(dir: &Path, doc: &Value, layer: &str, field: &str) -> Value {
    let Ok((_, l)) = crate::actions::layer(doc, layer) else { return Value::Null };
    let Some(id) = l["lighting"][field].as_str() else { return json!({ "map": null }) };
    let Some(image) = path_of(doc, id).and_then(|p| image::open(dir.join(p)).ok()) else {
        return Value::Null;
    };
    let mut shown = small(&image.into_rgba8());
    shown.pixels_mut().for_each(|p| {
        let [r, g, b] = crate::rigs::heat_rgb(f32::from(p.0[0]) / 255.0);
        *p = Rgba([r, g, b, 255]);
    });
    json!({ "map": id, "size": [shown.width(), shown.height()], "heat": crate::rigs::url(&shown) })
}
